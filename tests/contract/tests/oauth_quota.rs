use std::{
    fs,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
};

use any2api_contract_tests::{build_provider_registry, build_public_request_components};
use any2api_domain::{MaxConcurrency, OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_runtime::api::{
    ConfigPublisher, OAuthService, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, SqliteStore};
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
async fn codex_quota_query_and_credit_reset_are_protected_and_redacted() {
    let context = TestContext::new().await;
    let remote = SocketAddr::from(([203, 0, 113, 10], 41000));
    let response = request(
        context.app.clone(),
        Method::GET,
        &format!(
            "/api/admin/oauth/accounts/{}/quota",
            context.codex_account_id
        ),
        remote,
    )
    .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json["error"]["code"], "admin_loopback_only");
    assert_eq!(context.transport.calls(), 0);

    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let response = request(
        context.app.clone(),
        Method::GET,
        &format!(
            "/api/admin/oauth/accounts/{}/quota",
            context.codex_account_id
        ),
        loopback,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.cache_control.as_deref(), Some("no-store"));
    assert_eq!(response.json["rate_limit"]["allowed"], true);
    assert_eq!(
        response.json["rate_limit"]["primary_window"]["used_percent"],
        37.5
    );
    assert_eq!(response.json["reset_credits"]["available_count"], 1);
    assert_eq!(
        response.json["reset_credits"]["expires_at"],
        json!(["2026-07-30T00:00:00Z"])
    );
    assert!(response.json["fetched_at"].as_i64().is_some());
    let encoded = serde_json::to_string(&response.json).expect("quota JSON");
    for secret in [
        "access-secret",
        "refresh-secret",
        "account-123",
        "upstream-secret",
        "reset-credit-id",
    ] {
        assert!(!encoded.contains(secret));
    }

    let reset = request(
        context.app.clone(),
        Method::POST,
        &format!(
            "/api/admin/oauth/accounts/{}/quota/reset",
            context.codex_account_id
        ),
        loopback,
    )
    .await;
    assert_eq!(reset.status, StatusCode::OK);
    assert_eq!(reset.json, json!({"windows_reset": 2}));
    assert_eq!(context.transport.consume_calls(), 1);
    let redeem_request_id = context.transport.redeem_request_id();
    assert!(uuid::Uuid::parse_str(&redeem_request_id).is_ok());
    assert!(context.transport.all_requests_use_direct());
    assert!(context.transport.all_quota_headers_are_current());

    let exhausted = request(
        context.app.clone(),
        Method::POST,
        &format!(
            "/api/admin/oauth/accounts/{}/quota/reset",
            context.codex_account_id
        ),
        loopback,
    )
    .await;
    assert_eq!(exhausted.status, StatusCode::CONFLICT);
    assert_eq!(
        exhausted.json["error"]["code"],
        "oauth_quota_reset_unavailable"
    );
    assert_eq!(context.transport.consume_calls(), 1);

    let calls_before_claude = context.transport.calls();
    let unsupported = request(
        context.app,
        Method::GET,
        &format!(
            "/api/admin/oauth/accounts/{}/quota",
            context.claude_account_id
        ),
        loopback,
    )
    .await;
    assert_eq!(unsupported.status, StatusCode::BAD_REQUEST);
    assert_eq!(unsupported.json["error"]["code"], "oauth_quota_unsupported");
    assert_eq!(context.transport.calls(), calls_before_claude);
}

struct TestContext {
    _directory: tempfile::TempDir,
    _storage: Arc<SqliteStore>,
    app: Router,
    transport: Arc<QuotaTransport>,
    codex_account_id: OAuthAccountId,
    claude_account_id: OAuthAccountId,
}

impl TestContext {
    async fn new() -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("oauth-quota.sqlite3"))
                .await
                .expect("storage"),
        );
        let configuration = storage.load_configuration().await.expect("configuration");
        let providers = build_provider_registry();
        let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            providers.as_ref(),
        )));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                any2api_contract_tests::build_configuration_capabilities(),
            )
            .expect("publisher"),
        );
        let codex_account_id = OAuthAccountId::new();
        publisher
            .activate_oauth_account(
                codex_account_id,
                ProviderKind::Codex,
                draft("Codex OAuth"),
                Some("person@example.com".into()),
                None,
                vec!["gpt-5.5".into()],
                document(
                    ProviderKind::Codex,
                    br#"{"type":"codex","access_token":"access-secret","refresh_token":"refresh-secret","account_id":"account-123"}"#,
                ),
            )
            .await
            .expect("Codex account");
        let claude_account_id = OAuthAccountId::new();
        publisher
            .activate_oauth_account(
                claude_account_id,
                ProviderKind::Claude,
                draft("Claude OAuth"),
                None,
                None,
                vec!["claude-sonnet-4-5".into()],
                document(
                    ProviderKind::Claude,
                    br#"{"type":"claude","access_token":"claude-secret","refresh_token":"claude-refresh"}"#,
                ),
            )
            .await
            .expect("Claude account");
        let transport = Arc::new(QuotaTransport::new());
        let oauth = Arc::new(OAuthService::new(
            providers,
            Arc::clone(&transport) as Arc<dyn TransportManager>,
            Arc::clone(&publisher),
        ));
        let web_root = directory.path().join("web");
        fs::create_dir(&web_root).expect("web directory");
        fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
        let components = build_public_request_components().expect("public request components");
        let app = build_router(
            AppState::new(snapshots, runtime, publisher, components.service()).with_oauth(oauth),
            web_root,
        );
        Self {
            _directory: directory,
            _storage: storage,
            app,
            transport,
            codex_account_id,
            claude_account_id,
        }
    }
}

fn draft(label: &str) -> OAuthAccountDraft {
    OAuthAccountDraft::new(
        label,
        MaxConcurrency::new(2).expect("max concurrency"),
        true,
    )
    .expect("OAuth draft")
}

fn document(provider: ProviderKind, body: &'static [u8]) -> OAuthAccountDocument {
    OAuthAccountDocument::new(provider, body.to_vec().into()).expect("OAuth document")
}

struct CapturedRequest {
    path: String,
    authorization: Option<String>,
    account_id: Option<String>,
    proxy_id: any2api_domain::ProxyProfileId,
}

struct QuotaTransport {
    available_count: AtomicU32,
    consume_calls: AtomicUsize,
    captured: Mutex<Vec<CapturedRequest>>,
    redeem_request_id: Mutex<Option<String>>,
}

impl QuotaTransport {
    fn new() -> Self {
        Self {
            available_count: AtomicU32::new(1),
            consume_calls: AtomicUsize::new(0),
            captured: Mutex::new(Vec::new()),
            redeem_request_id: Mutex::new(None),
        }
    }

    fn calls(&self) -> usize {
        self.captured.lock().expect("captured lock").len()
    }

    fn consume_calls(&self) -> usize {
        self.consume_calls.load(Ordering::Acquire)
    }

    fn redeem_request_id(&self) -> String {
        self.redeem_request_id
            .lock()
            .expect("redeem request lock")
            .clone()
            .expect("redeem request id")
    }

    fn all_requests_use_direct(&self) -> bool {
        self.captured
            .lock()
            .expect("captured lock")
            .iter()
            .all(|request| request.proxy_id == any2api_domain::ProxyProfileId::DIRECT)
    }

    fn all_quota_headers_are_current(&self) -> bool {
        self.captured
            .lock()
            .expect("captured lock")
            .iter()
            .all(|request| {
                request.authorization.as_deref() == Some("Bearer access-secret")
                    && request.account_id.as_deref() == Some("account-123")
                    && request.path.starts_with("/backend-api/wham/")
            })
    }
}

#[async_trait]
impl TransportManager for QuotaTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let path = request.uri.path().to_owned();
        self.captured
            .lock()
            .expect("captured lock")
            .push(CapturedRequest {
                path: path.clone(),
                authorization: request
                    .headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                account_id: request
                    .headers
                    .get("chatgpt-account-id")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                proxy_id: proxy.profile().id(),
            });
        let body = match path.as_str() {
            "/backend-api/wham/usage" => Bytes::from_static(
                br#"{"user_id":"upstream-secret","account_id":"account-123","rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":37.5,"limit_window_seconds":18000,"reset_after_seconds":300,"reset_at":1900000000},"secondary_window":null},"rate_limit_reset_credits":{"available_count":9}}"#,
            ),
            "/backend-api/wham/rate-limit-reset-credits" => Bytes::from(
                serde_json::json!({
                    "available_count": self.available_count.load(Ordering::Acquire),
                    "credits": [{
                        "id": "reset-credit-id",
                        "reset_type": "codex_rate_limits",
                        "status": "available",
                        "expires_at": "2026-07-30T00:00:00Z"
                    }]
                })
                .to_string(),
            ),
            "/backend-api/wham/rate-limit-reset-credits/consume" => {
                let body: Value = serde_json::from_slice(&request.body).expect("reset request");
                *self
                    .redeem_request_id
                    .lock()
                    .expect("redeem request lock") = body["redeem_request_id"]
                    .as_str()
                    .map(str::to_owned);
                self.available_count.store(0, Ordering::Release);
                self.consume_calls.fetch_add(1, Ordering::AcqRel);
                Bytes::from_static(
                    br#"{"code":"ok","windows_reset":2,"credit":{"id":"reset-credit-id"}}"#,
                )
            }
            other => panic!("unexpected path: {other}"),
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
