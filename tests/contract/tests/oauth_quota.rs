use std::{
    fs,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicUsize, Ordering},
    },
};

use any2api_contract_tests::{build_provider_registry, build_public_request_components};
use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind};
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
        response.json["rate_limit"]["windows"][0]["used_percent"],
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
}

#[tokio::test]
async fn grok_unified_billing_returns_redacted_request_and_token_windows() {
    let context = TestContext::new().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let response = request(
        context.app,
        Method::GET,
        &format!(
            "/api/admin/oauth/accounts/{}/quota",
            context.grok_account_id
        ),
        loopback,
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.cache_control.as_deref(), Some("no-store"));
    assert_eq!(
        response.json["rate_limit"]["windows"][0]["kind"],
        "requests"
    );
    assert_eq!(
        response.json["rate_limit"]["windows"][0]["used_percent"],
        25.0
    );
    assert!(response.json["rate_limit"]["windows"][0]["limit_window_seconds"].is_null());
    assert_eq!(response.json["rate_limit"]["windows"][1]["kind"], "tokens");
    assert_eq!(
        response.json["rate_limit"]["windows"][1]["used_percent"],
        60.0
    );
    assert!(response.json["reset_credits"].is_null());
    assert_eq!(context.transport.calls(), 2);
    assert!(context.transport.last_requests_are_grok_hybrid_query());
    let encoded = serde_json::to_string(&response.json).expect("Grok quota JSON");
    assert!(!encoded.contains("grok-access-secret"));
    assert!(!encoded.contains("grok-refresh-secret"));
}

#[tokio::test]
async fn claude_quota_returns_all_redacted_usage_windows() {
    let context = TestContext::new().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let response = request(
        context.app,
        Method::GET,
        &format!(
            "/api/admin/oauth/accounts/{}/quota",
            context.claude_account_id
        ),
        loopback,
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.cache_control.as_deref(), Some("no-store"));
    assert!(response.json["rate_limit"]["allowed"].is_null());
    assert!(response.json["rate_limit"]["limit_reached"].is_null());
    let windows = response.json["rate_limit"]["windows"]
        .as_array()
        .expect("quota windows");
    assert_eq!(windows.len(), 4);
    assert_eq!(windows[0]["id"], "five_hour");
    assert_eq!(windows[0]["used_percent"], 12.5);
    assert_eq!(windows[1]["id"], "seven_day");
    assert_eq!(windows[2]["id"], "seven_day_sonnet");
    assert_eq!(windows[3]["id"], "seven_day_overage_included");
    assert!(response.json["reset_credits"].is_null());
    assert_eq!(context.transport.calls(), 1);
    assert!(context.transport.last_request_is_claude_usage());
    let encoded = serde_json::to_string(&response.json).expect("Claude quota JSON");
    assert!(!encoded.contains("claude-secret"));
    assert!(!encoded.contains("claude-refresh"));
    assert!(!encoded.contains("upstream-secret"));
}

struct TestContext {
    _directory: tempfile::TempDir,
    _storage: Arc<SqliteStore>,
    app: Router,
    transport: Arc<QuotaTransport>,
    codex_account_id: OAuthAccountId,
    grok_account_id: OAuthAccountId,
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
        let runtime = Arc::new(RuntimeRegistry::new());
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
        let grok_account_id = OAuthAccountId::new();
        publisher
            .activate_oauth_account(
                grok_account_id,
                ProviderKind::Grok,
                draft("Grok OAuth"),
                Some("grok@example.com".into()),
                None,
                vec!["grok-4.5".into()],
                document(
                    ProviderKind::Grok,
                    br#"{"type":"grok","access_token":"grok-access-secret","refresh_token":"grok-refresh-secret","sub":"grok-subject"}"#,
                ),
            )
            .await
            .expect("Grok account");
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
            grok_account_id,
            claude_account_id,
        }
    }
}

fn draft(label: &str) -> OAuthAccountDraft {
    OAuthAccountDraft::new(label, None, true).expect("OAuth draft")
}

fn document(provider: ProviderKind, body: &'static [u8]) -> OAuthAccountDocument {
    OAuthAccountDocument::new(provider, body.to_vec().into()).expect("OAuth document")
}

struct CapturedRequest {
    method: Method,
    path: String,
    path_and_query: String,
    authorization: Option<String>,
    account_id: Option<String>,
    grok_token_auth: Option<String>,
    grok_client_version: Option<String>,
    anthropic_beta: Option<String>,
    user_agent: Option<String>,
    proxy_id: any2api_domain::ProxyProfileId,
    body: Bytes,
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

    fn last_requests_are_grok_hybrid_query(&self) -> bool {
        let captured = self.captured.lock().expect("captured lock");
        let [billing, probe] = captured.as_slice() else {
            return false;
        };
        let probe_body = serde_json::from_slice::<Value>(&probe.body).ok();
        billing.method == Method::GET
            && billing.path_and_query == "/v1/billing?format=credits"
            && probe.method == Method::POST
            && probe.path_and_query == "/v1/responses"
            && probe_body == Some(json!({"model":"grok-4.5","input":"hi","stream":true}))
            && [billing, probe].iter().all(|request| {
                request.authorization.as_deref() == Some("Bearer grok-access-secret")
                    && request.account_id.is_none()
                    && request.grok_token_auth.as_deref() == Some("xai-grok-cli")
                    && request.grok_client_version.as_deref() == Some("0.2.93")
                    && request.proxy_id == any2api_domain::ProxyProfileId::DIRECT
            })
    }

    fn last_request_is_claude_usage(&self) -> bool {
        self.captured
            .lock()
            .expect("captured lock")
            .as_slice()
            .first()
            .is_some_and(|request| {
                request.method == Method::GET
                    && request.path == "/api/oauth/usage"
                    && request.authorization.as_deref() == Some("Bearer claude-secret")
                    && request.anthropic_beta.as_deref() == Some("oauth-2025-04-20")
                    && request.user_agent.as_deref() == Some("claude-code/2.1.7")
                    && request.proxy_id == any2api_domain::ProxyProfileId::DIRECT
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
        let path_and_query = request
            .uri
            .path_and_query()
            .map_or_else(|| path.clone(), ToString::to_string);
        self.captured
            .lock()
            .expect("captured lock")
            .push(CapturedRequest {
                method: request.method.clone(),
                path: path.clone(),
                path_and_query,
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
                grok_token_auth: request
                    .headers
                    .get("x-xai-token-auth")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                grok_client_version: request
                    .headers
                    .get("x-grok-client-version")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                anthropic_beta: request
                    .headers
                    .get("anthropic-beta")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                user_agent: request
                    .headers
                    .get("user-agent")
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned),
                proxy_id: proxy.profile().id(),
                body: request.body.clone(),
            });
        let (body, headers) = match path.as_str() {
            "/backend-api/wham/usage" => (Bytes::from_static(
                br#"{"user_id":"upstream-secret","account_id":"account-123","rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":37.5,"limit_window_seconds":18000,"reset_after_seconds":300,"reset_at":1900000000},"secondary_window":null},"rate_limit_reset_credits":{"available_count":9}}"#,
            ), HeaderMap::new()),
            "/backend-api/wham/rate-limit-reset-credits" => (Bytes::from(
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
            ), HeaderMap::new()),
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
                (Bytes::from_static(
                    br#"{"code":"ok","windows_reset":2,"credit":{"id":"reset-credit-id"}}"#,
                ), HeaderMap::new())
            }
            "/v1/billing" => (Bytes::from_static(
                br#"{"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"2030-01-01T00:00:00Z","end":"2030-01-08T00:00:00Z"},"onDemandCap":{"val":"0"},"onDemandUsed":{"val":"0"},"isUnifiedBillingUser":true,"prepaidBalance":{"val":"0"}}}"#,
            ), HeaderMap::new()),
            "/v1/responses" => {
                let mut headers = HeaderMap::new();
                for (name, value) in [
                    ("x-ratelimit-limit-requests", "100"),
                    ("x-ratelimit-remaining-requests", "75"),
                    ("x-ratelimit-reset-requests", "1900000000"),
                    ("x-ratelimit-limit-tokens", "1000"),
                    ("x-ratelimit-remaining-tokens", "400"),
                    ("x-ratelimit-reset-tokens", "1900000000"),
                ] {
                    headers.insert(
                        http::header::HeaderName::from_bytes(name.as_bytes()).expect("header"),
                        http::HeaderValue::from_static(value),
                    );
                }
                (Bytes::new(), headers)
            }
            "/api/oauth/usage" => (Bytes::from_static(
                br#"{"five_hour":{"utilization":12.5,"resets_at":"2030-01-01T01:00:00Z"},"seven_day":{"utilization":34.0,"resets_at":"2030-01-08T00:00:00Z"},"seven_day_sonnet":{"utilization":56.0,"resets_at":"2030-01-08T00:00:00Z"},"seven_day_overage_included":{"utilization":78.0,"resets_at":"2030-01-08T00:00:00Z"},"unknown":{"token":"upstream-secret"}}"#,
            ), HeaderMap::new()),
            other => panic!("unexpected path: {other}"),
        };
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers,
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
