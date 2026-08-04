use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
};

use any2api_contract_tests::TestApplication;
use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind, ProxyProfileId};
use any2api_runtime::api::OAuthService;
use any2api_storage::api::OAuthAccountDocument;
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Method, Request, StatusCode, header::CACHE_CONTROL},
};
use bytes::Bytes;
use futures_util::stream;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn codex_quota_is_persisted_redacted_reset_and_announced() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let publisher = fixture.publisher();
    let account_id = OAuthAccountId::new();
    publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            OAuthAccountDraft::new("Codex OAuth", None, true).expect("OAuth draft"),
            Some("person@example.com".into()),
            None,
            vec!["gpt-5.5".into()],
            OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"access_token":"access-secret","refresh_token":"refresh-secret","id_token":null,"account_id":"account-123","email":"person@example.com"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        )
        .await
        .expect("Codex account");
    let transport = Arc::new(QuotaTransport {
        available: AtomicU32::new(1),
        ..QuotaTransport::default()
    });
    let oauth = Arc::new(OAuthService::new(
        fixture.components().provider_registry_handle(),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
        publisher,
        storage,
    ));
    let state = fixture.state().with_oauth(oauth);
    let (_directory, app, _) = fixture.into_router_with_state(state);
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let quota_uri = format!("/api/admin/oauth/accounts/{account_id}/quota");

    let cached = request(app.clone(), Method::GET, &quota_uri, loopback).await;
    assert_eq!(cached.status, StatusCode::OK);
    assert!(cached.json.is_null());
    assert_eq!(transport.calls.load(Ordering::Acquire), 0);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/admin/oauth/quota-events")
                .extension(ConnectInfo(loopback))
                .body(Body::empty())
                .expect("quota event request"),
        )
        .await
        .expect("quota event response");
    assert_eq!(response.status(), StatusCode::OK);
    let mut events = response.into_body();
    assert!(next_sse_event(&mut events).await.contains("data: 0"));

    let refreshed = request(
        app.clone(),
        Method::POST,
        &format!("{quota_uri}/refresh"),
        loopback,
    )
    .await;
    assert_eq!(refreshed.status, StatusCode::OK);
    assert_eq!(refreshed.cache_control.as_deref(), Some("no-store"));
    assert_eq!(
        refreshed.json["rate_limit"]["windows"][0]["used_percent"],
        37.5
    );
    assert_eq!(refreshed.json["reset_credits"]["available_count"], 1);
    let encoded = refreshed.json.to_string();
    for secret in [
        "access-secret",
        "refresh-secret",
        "account-123",
        "credit-id",
    ] {
        assert!(!encoded.contains(secret));
    }
    assert_eq!(transport.calls.load(Ordering::Acquire), 2);
    assert!(next_sse_event(&mut events).await.contains("data: 1"));

    let persisted = request(app.clone(), Method::GET, &quota_uri, loopback).await;
    assert_eq!(persisted.json["fetched_at"], refreshed.json["fetched_at"]);
    assert_eq!(transport.calls.load(Ordering::Acquire), 2);

    let reset = request(
        app.clone(),
        Method::POST,
        &format!("{quota_uri}/reset"),
        loopback,
    )
    .await;
    assert_eq!(reset.status, StatusCode::OK);
    assert_eq!(reset.json, json!({"windows_reset": 2}));
    assert_eq!(transport.consumed.load(Ordering::Acquire), 1);
    assert!(
        request(app, Method::GET, &quota_uri, loopback)
            .await
            .json
            .is_null()
    );
}

#[derive(Default)]
struct QuotaTransport {
    calls: AtomicUsize,
    consumed: AtomicUsize,
    available: AtomicU32,
}

#[async_trait]
impl TransportManager for QuotaTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        assert_eq!(request.headers["authorization"], "Bearer access-secret");
        assert_eq!(request.headers["chatgpt-account-id"], "account-123");
        self.calls.fetch_add(1, Ordering::AcqRel);
        let body = match request.uri.path() {
            "/backend-api/wham/usage" => Bytes::from_static(
                br#"{"rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":37.5,"limit_window_seconds":18000,"reset_after_seconds":300,"reset_at":1900000000},"secondary_window":null}}"#,
            ),
            "/backend-api/wham/rate-limit-reset-credits" => Bytes::from(
                json!({
                    "available_count": self.available.load(Ordering::Acquire),
                    "credits": [{
                        "id": "credit-id",
                        "reset_type": "codex_rate_limits",
                        "status": "available",
                        "expires_at": "2026-07-30T00:00:00Z"
                    }]
                })
                .to_string(),
            ),
            "/backend-api/wham/rate-limit-reset-credits/consume" => {
                self.available.store(0, Ordering::Release);
                self.consumed.fetch_add(1, Ordering::AcqRel);
                Bytes::from_static(br#"{"code":"ok","windows_reset":2}"#)
            }
            path => panic!("unexpected quota path: {path}"),
        };
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

struct ResponseBody {
    status: StatusCode,
    cache_control: Option<String>,
    json: Value,
}

async fn request(app: Router, method: Method, uri: &str, remote: SocketAddr) -> ResponseBody {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .extension(ConnectInfo(remote))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let cache_control = response
        .headers()
        .get(CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let json = serde_json::from_slice(
        &response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes(),
    )
    .expect("response JSON");
    ResponseBody {
        status,
        cache_control,
        json,
    }
}

async fn next_sse_event(body: &mut Body) -> String {
    let frame = tokio::time::timeout(std::time::Duration::from_secs(1), body.frame())
        .await
        .expect("SSE frame timeout")
        .expect("SSE frame")
        .expect("valid SSE frame");
    String::from_utf8(frame.into_data().expect("SSE data frame").to_vec()).expect("UTF-8 SSE event")
}
