use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_contract_tests::TestApplication;
use any2api_domain::{OAuthProxySelection, ProviderKind, ProxyProfileId};
use any2api_provider::{CodexDriver, GrokDriver, api::ProviderRegistry};
use any2api_runtime::api::{OAuthService, RequestTelemetry};
use any2api_storage::api::ConfigurationRepository;
use any2api_transport::api::{
    TransportFailureScope, TransportManager, TransportProxy, TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Method, Request, StatusCode, header::CONTENT_TYPE},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use tower::ServiceExt;

#[tokio::test]
async fn oauth_http_flows_activate_redacted_codex_and_grok_accounts() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");
    let oauth = Arc::new(OAuthService::new(
        Arc::new(providers),
        Arc::new(TokenTransport),
        fixture.publisher(),
        Arc::clone(&storage),
        Arc::new(RequestTelemetry::disabled()),
    ));
    let state = fixture.state().with_oauth(oauth);
    let (_directory, app, _) = fixture.into_router_with_state(state);
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, codex_start) = request_json(
        app.clone(),
        "/api/admin/oauth/start",
        json!({
            "provider": "codex",
            "proxy_selection": {"mode": "global"}
        }),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let authorization_url = codex_start["authorization_url"]
        .as_str()
        .expect("authorization URL");
    let state = url::Url::parse(authorization_url)
        .expect("authorization URL")
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("OAuth state");
    let (status, codex) = request_json(
        app.clone(),
        "/api/admin/oauth/exchange",
        json!({
            "session_id": codex_start["session_id"],
            "callback_url": format!(
                "http://localhost:1455/auth/callback?code=authorization-code&state={state}"
            )
        }),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(codex["provider"], "codex");
    assert_eq!(codex["proxy_selection"]["mode"], "global");
    assert_eq!(codex["safe_account_email"], "person@example.com");
    assert_redacted(&codex);

    let (status, grok_start) = request_json(
        app.clone(),
        "/api/admin/oauth/start",
        json!({
            "provider": "grok",
            "proxy_selection": {"mode": "global"}
        }),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grok_start["flow"], "device_code");
    assert_eq!(grok_start["user_code"], "ABCD-1234");
    assert!(grok_start.get("device_code").is_none());
    let (status, grok) = request_json(
        app,
        "/api/admin/oauth/device/poll",
        json!({"session_id": grok_start["session_id"]}),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grok["status"], "complete");
    assert_eq!(grok["account"]["provider"], "grok");
    assert_eq!(grok["account"]["proxy_selection"]["mode"], "global");
    assert_redacted(&grok);

    let configuration = storage.load_configuration().await.expect("configuration");
    assert_eq!(configuration.revision().get(), 3);
    assert_eq!(configuration.oauth_accounts().accounts().len(), 2);
}

#[tokio::test]
async fn oauth_service_paces_concurrent_network_starts_for_one_provider() {
    let fixture = TestApplication::new().await;
    let transport = Arc::new(PacingTransport::new());
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");
    let oauth = Arc::new(OAuthService::new(
        Arc::new(providers),
        transport.clone(),
        fixture.publisher(),
        fixture.storage(),
        Arc::new(RequestTelemetry::disabled()),
    ));
    tokio::time::pause();

    let first = {
        let oauth = Arc::clone(&oauth);
        tokio::spawn(async move {
            oauth
                .start(ProviderKind::Grok, OAuthProxySelection::Global)
                .await
        })
    };
    let second = {
        let oauth = Arc::clone(&oauth);
        tokio::spawn(async move {
            oauth
                .start(ProviderKind::Grok, OAuthProxySelection::Global)
                .await
        })
    };
    transport.wait_for_start().await;
    tokio::task::yield_now().await;
    assert_eq!(transport.calls(), 1);

    tokio::time::advance(Duration::from_millis(499)).await;
    tokio::task::yield_now().await;
    assert_eq!(transport.calls(), 1);

    tokio::time::advance(Duration::from_millis(1)).await;
    transport.wait_for_start().await;
    assert_eq!(transport.calls(), 2);
    first.await.expect("first task").expect("first OAuth start");
    second
        .await
        .expect("second task")
        .expect("second OAuth start");
}

fn assert_redacted(value: &Value) {
    let encoded = value.to_string();
    for secret in [
        "codex-access-token",
        "codex-refresh-token",
        "device-secret",
        "grok-access-token",
        "grok-refresh-token",
    ] {
        assert!(!encoded.contains(secret));
    }
}

async fn request_json(
    app: Router,
    uri: &str,
    body: Value,
    remote: SocketAddr,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .extension(ConnectInfo(remote))
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).expect("request JSON")))
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).expect("response JSON"),
    )
}

struct TokenTransport;

#[async_trait]
impl TransportManager for TokenTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        let body = match (request.uri.host(), request.uri.path()) {
            (Some("auth.openai.com"), "/oauth/token") => Bytes::from_static(
                br#"{"access_token":"codex-access-token","refresh_token":"codex-refresh-token","id_token":"header.eyJlbWFpbCI6InBlcnNvbkBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NvdW50LTEyMyJ9fQ.signature","expires_in":3600}"#,
            ),
            (Some("chatgpt.com"), "/backend-api/codex/models") => Bytes::from_static(
                br#"{"models":[{"slug":"gpt-5.5","supported_in_api":true}]}"#,
            ),
            (Some("auth.x.ai"), "/oauth2/device/code") => Bytes::from_static(
                br#"{"device_code":"device-secret","user_code":"ABCD-1234","verification_uri":"https://accounts.x.ai/oauth2/device","expires_in":1800,"interval":5}"#,
            ),
            (Some("auth.x.ai"), "/oauth2/token") => Bytes::from_static(
                br#"{"access_token":"grok-access-token","refresh_token":"grok-refresh-token","id_token":"header.eyJlbWFpbCI6Imdyb2tAZXhhbXBsZS5jb20iLCJzdWIiOiJncm9rLXN1YmplY3QifQ.signature","expires_in":3600}"#,
            ),
            (Some("cli-chat-proxy.grok.com"), "/v1/models") => {
                Bytes::from_static(br#"{"data":[{"id":"grok-4"}]}"#)
            }
            target => panic!("unexpected OAuth request target: {target:?}"),
        };
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async { Ok(body) })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

struct PacingTransport {
    calls: AtomicUsize,
    started: Semaphore,
}

impl PacingTransport {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            started: Semaphore::new(0),
        }
    }

    async fn wait_for_start(&self) {
        self.started
            .acquire()
            .await
            .expect("OAuth start signal")
            .forget();
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }
}

#[async_trait]
impl TransportManager for PacingTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.started.add_permits(1);
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async {
                Ok(Bytes::from_static(
                    br#"{"device_code":"device-secret","user_code":"ABCD-1234","verification_uri":"https://accounts.x.ai/oauth2/device","expires_in":1800,"interval":5}"#,
                ))
            })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
