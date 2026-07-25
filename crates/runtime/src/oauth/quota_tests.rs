use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};

use any2api_domain::{
    OAuthAccountDraft, OAuthAccountId, ProviderKind, RetrySafety, RoutingCredentialId,
    SettingsConfiguration, UpstreamErrorClassification, UpstreamErrorKind,
};
use any2api_provider::{CodexDriver, GrokDriver, ProviderRegistry};
use any2api_storage::api::{
    ConfigurationRepository, OAuthAccountDocument, OAuthAccountRepository, SqliteStore,
};
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use tokio::sync::Semaphore;

use super::{OAuthQuotaError, OAuthService};
use crate::{
    health::{HealthAcquireError, ReliabilityPolicy},
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn query_and_reset_use_direct_transport_and_clear_temporary_cooldowns() {
    let context = QuotaTestContext::new(1, AuthenticationMode::Accepted).await;
    let quota = context
        .service
        .query_quota(context.account_id)
        .await
        .expect("quota query");
    assert_eq!(
        quota
            .usage
            .rate_limit
            .as_ref()
            .and_then(|limit| limit.primary_window.as_ref())
            .map(|window| window.used_percent),
        Some(25.0)
    );
    assert_eq!(
        quota
            .usage
            .reset_credits
            .as_ref()
            .map(|credits| credits.available_count),
        Some(1)
    );

    let snapshot = context.snapshots.load();
    let generation = Arc::clone(
        snapshot
            .credential_runtime(RoutingCredentialId::oauth_account(context.account_id))
            .expect("OAuth runtime")
            .generation(),
    );
    generation.health().record(
        "gpt-5.5",
        UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            None,
        ),
        &ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability()),
    );
    assert!(matches!(
        generation.health().availability("gpt-5.5"),
        Err(HealthAcquireError::Temporary(_))
    ));
    let epoch_before = context.runtime.scheduler_epoch();
    drop(snapshot);

    let reset = context
        .service
        .reset_quota(context.account_id)
        .await
        .expect("quota reset");
    assert_eq!(reset.windows_reset, 2);
    assert_eq!(generation.health().availability("gpt-5.5"), Ok(()));
    assert!(context.runtime.scheduler_epoch() > epoch_before);

    let captured = context.transport.captured();
    assert_eq!(
        captured
            .iter()
            .map(|request| request.path.as_str())
            .collect::<Vec<_>>(),
        [
            "/backend-api/wham/usage",
            "/backend-api/wham/rate-limit-reset-credits",
            "/backend-api/wham/usage",
            "/backend-api/wham/rate-limit-reset-credits",
            "/backend-api/wham/rate-limit-reset-credits/consume",
        ]
    );
    assert!(captured.iter().all(|request| {
        request.proxy_id == any2api_domain::ProxyProfileId::DIRECT
            && request.account_id.as_deref() == Some("account-123")
            && request.strict_ssrf == context.snapshots.load().settings().upstream().strict_ssrf()
    }));
    let redeem_id = serde_json::from_slice::<serde_json::Value>(
        &captured.last().expect("consume request").body,
    )
    .expect("consume body")["redeem_request_id"]
        .as_str()
        .expect("redeem request id")
        .to_owned();
    assert!(uuid::Uuid::parse_str(&redeem_id).is_ok());
}

#[tokio::test]
async fn grok_query_uses_one_direct_billing_request_without_reset_credits() {
    let context = QuotaTestContext::new_grok().await;

    let quota = context
        .service
        .query_quota(context.account_id)
        .await
        .expect("Grok quota query");

    assert_eq!(
        quota
            .usage
            .rate_limit
            .as_ref()
            .and_then(|limit| limit.primary_window.as_ref())
            .map(|window| (window.used_percent, window.limit_window_seconds)),
        Some((37.5, 604_800))
    );
    assert!(quota.usage.reset_credits.is_none());
    let captured = context.transport.captured();
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    assert_eq!(request.path, "/v1/billing?format=credits");
    assert_eq!(request.authorization.as_deref(), Some("Bearer grok-access"));
    assert_eq!(request.grok_token_auth.as_deref(), Some("xai-grok-cli"));
    assert_eq!(request.grok_client_version.as_deref(), Some("0.2.93"));
    assert!(request.account_id.is_none());
    assert_eq!(request.proxy_id, any2api_domain::ProxyProfileId::DIRECT);
    assert_eq!(
        request.strict_ssrf,
        context.snapshots.load().settings().upstream().strict_ssrf()
    );
}

#[tokio::test]
async fn reset_without_available_credit_never_calls_consume() {
    let context = QuotaTestContext::new(0, AuthenticationMode::Accepted).await;

    assert!(matches!(
        context.service.reset_quota(context.account_id).await,
        Err(OAuthQuotaError::NoResetCredits)
    ));
    assert_eq!(context.transport.consume_calls(), 0);
}

#[tokio::test]
async fn quota_query_refreshes_once_after_authentication_rejection() {
    let context = QuotaTestContext::new(1, AuthenticationMode::RejectOnce).await;

    context
        .service
        .query_quota(context.account_id)
        .await
        .expect("quota query after refresh");

    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(
        context.transport.usage_authorizations(),
        ["Bearer old-access", "Bearer new-access"]
    );
    assert_eq!(
        context
            .snapshots
            .load()
            .oauth_accounts()
            .get(context.account_id)
            .expect("OAuth account")
            .token_version(),
        2
    );
}

#[tokio::test]
async fn a_second_quota_401_does_not_refresh_or_query_a_third_time() {
    let context = QuotaTestContext::new(1, AuthenticationMode::AlwaysReject).await;

    assert!(matches!(
        context.service.query_quota(context.account_id).await,
        Err(OAuthQuotaError::AuthenticationFailed)
    ));
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(context.transport.usage_authorizations().len(), 2);
}

#[tokio::test]
async fn concurrent_resets_serialize_and_only_consume_the_last_credit_once() {
    let context = QuotaTestContext::new_blocking_reset(1).await;
    let first_service = Arc::clone(&context.service);
    let id = context.account_id;
    let first = tokio::spawn(async move { first_service.reset_quota(id).await });
    context.transport.wait_for_consume().await;

    let second_service = Arc::clone(&context.service);
    let second = tokio::spawn(async move { second_service.reset_quota(id).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert_eq!(context.transport.usage_calls(), 1);
    context.transport.release_consume();

    assert_eq!(
        first
            .await
            .expect("first reset")
            .expect("reset result")
            .windows_reset,
        2
    );
    assert!(matches!(
        second.await.expect("second reset"),
        Err(OAuthQuotaError::NoResetCredits)
    ));
    assert_eq!(context.transport.consume_calls(), 1);
    assert_eq!(context.transport.usage_calls(), 2);
}

struct QuotaTestContext {
    _directory: tempfile::TempDir,
    _storage: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    service: Arc<OAuthService>,
    transport: Arc<QuotaTransport>,
    account_id: OAuthAccountId,
}

impl QuotaTestContext {
    async fn new(available_count: u32, authentication: AuthenticationMode) -> Self {
        Self::with_transport(Arc::new(QuotaTransport::new(
            available_count,
            authentication,
            false,
        )))
        .await
    }

    async fn new_blocking_reset(available_count: u32) -> Self {
        Self::with_transport(Arc::new(QuotaTransport::new(
            available_count,
            AuthenticationMode::Accepted,
            true,
        )))
        .await
    }

    async fn new_grok() -> Self {
        Self::with_account(
            Arc::new(QuotaTransport::new(0, AuthenticationMode::Accepted, false)),
            ProviderKind::Grok,
            "Grok OAuth",
            None,
            vec!["grok-4.5".into()],
            grok_oauth_document(),
        )
        .await
    }

    async fn with_transport(transport: Arc<QuotaTransport>) -> Self {
        Self::with_account(
            transport,
            ProviderKind::Codex,
            "Codex OAuth",
            Some("person@example.com".into()),
            vec!["gpt-5.5".into()],
            oauth_document(),
        )
        .await
    }

    async fn with_account(
        transport: Arc<QuotaTransport>,
        provider: ProviderKind,
        label: &str,
        email: Option<String>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("oauth-quota.sqlite3"))
                .await
                .expect("storage"),
        );
        let initial = storage.load_configuration().await.expect("configuration");
        let account_id = OAuthAccountId::new();
        let configured = storage
            .create_oauth_account(
                initial.revision(),
                account_id,
                provider,
                OAuthAccountDraft::new(label, None, true).expect("OAuth draft"),
                email,
                None,
                models,
                document,
            )
            .await
            .expect("OAuth account");
        let providers = providers();
        let runtime = Arc::new(RuntimeRegistry::new());
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            configured,
            runtime.as_ref(),
            providers.as_ref(),
        )));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                crate::test_support::configuration_capabilities(),
            )
            .expect("publisher"),
        );
        let service = Arc::new(OAuthService::new(
            providers,
            Arc::clone(&transport) as Arc<dyn TransportManager>,
            publisher,
        ));
        Self {
            _directory: directory,
            _storage: storage,
            snapshots,
            runtime,
            service,
            transport,
            account_id,
        }
    }
}

fn providers() -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");
    Arc::new(providers)
}

fn oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        br#"{"type":"codex","access_token":"old-access","refresh_token":"old-refresh","account_id":"account-123"}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}

fn grok_oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Grok,
        br#"{"type":"grok","access_token":"grok-access","refresh_token":"grok-refresh","sub":"grok-subject"}"#
            .to_vec()
            .into(),
    )
    .expect("Grok OAuth document")
}

#[derive(Clone, Copy)]
enum AuthenticationMode {
    Accepted,
    RejectOnce,
    AlwaysReject,
}

struct CapturedQuotaRequest {
    path: String,
    authorization: Option<String>,
    account_id: Option<String>,
    grok_token_auth: Option<String>,
    grok_client_version: Option<String>,
    proxy_id: any2api_domain::ProxyProfileId,
    strict_ssrf: bool,
    body: Bytes,
}

struct QuotaTransport {
    available_count: AtomicU32,
    authentication: AuthenticationMode,
    usage_calls: AtomicUsize,
    refresh_calls: AtomicUsize,
    consume_calls: AtomicUsize,
    block_consume: bool,
    consume_started: Semaphore,
    consume_release: Semaphore,
    captured: Mutex<Vec<CapturedQuotaRequest>>,
}

impl QuotaTransport {
    fn new(available_count: u32, authentication: AuthenticationMode, block_consume: bool) -> Self {
        Self {
            available_count: AtomicU32::new(available_count),
            authentication,
            usage_calls: AtomicUsize::new(0),
            refresh_calls: AtomicUsize::new(0),
            consume_calls: AtomicUsize::new(0),
            block_consume,
            consume_started: Semaphore::new(0),
            consume_release: Semaphore::new(0),
            captured: Mutex::new(Vec::new()),
        }
    }

    fn captured(&self) -> Vec<CapturedQuotaRequest> {
        self.captured
            .lock()
            .expect("captured request lock")
            .iter()
            .map(|request| CapturedQuotaRequest {
                path: request.path.clone(),
                authorization: request.authorization.clone(),
                account_id: request.account_id.clone(),
                grok_token_auth: request.grok_token_auth.clone(),
                grok_client_version: request.grok_client_version.clone(),
                proxy_id: request.proxy_id,
                strict_ssrf: request.strict_ssrf,
                body: request.body.clone(),
            })
            .collect()
    }

    fn refresh_calls(&self) -> usize {
        self.refresh_calls.load(Ordering::Acquire)
    }

    fn consume_calls(&self) -> usize {
        self.consume_calls.load(Ordering::Acquire)
    }

    fn usage_calls(&self) -> usize {
        self.usage_calls.load(Ordering::Acquire)
    }

    async fn wait_for_consume(&self) {
        self.consume_started
            .acquire()
            .await
            .expect("consume start signal")
            .forget();
    }

    fn release_consume(&self) {
        self.consume_release.add_permits(1);
    }

    fn usage_authorizations(&self) -> Vec<String> {
        self.captured()
            .into_iter()
            .filter(|request| request.path.ends_with("/usage"))
            .filter_map(|request| request.authorization)
            .collect()
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
        let captured_path = request
            .uri
            .path_and_query()
            .map_or_else(|| path.clone(), ToString::to_string);
        let authorization = request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        self.captured
            .lock()
            .expect("captured request lock")
            .push(CapturedQuotaRequest {
                path: captured_path,
                authorization,
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
                proxy_id: proxy.profile().id(),
                strict_ssrf: request.network_policy.strict_ssrf(),
                body: request.body,
            });
        let (status, body) = match path.as_str() {
            "/oauth/token" => {
                self.refresh_calls.fetch_add(1, Ordering::AcqRel);
                (
                    StatusCode::OK,
                    Bytes::from_static(br#"{"access_token":"new-access","expires_in":3600}"#),
                )
            }
            "/backend-api/wham/usage" => {
                let attempt = self.usage_calls.fetch_add(1, Ordering::AcqRel);
                let rejected = matches!(self.authentication, AuthenticationMode::AlwaysReject)
                    || matches!(self.authentication, AuthenticationMode::RejectOnce)
                        && attempt == 0;
                if rejected {
                    (StatusCode::UNAUTHORIZED, Bytes::from_static(b"{}"))
                } else {
                    (
                        StatusCode::OK,
                        Bytes::from_static(
                            br#"{"rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":25.0,"limit_window_seconds":18000,"reset_after_seconds":60,"reset_at":1900000000},"secondary_window":null},"rate_limit_reset_credits":{"available_count":7}}"#,
                        ),
                    )
                }
            }
            "/v1/billing" => {
                self.usage_calls.fetch_add(1, Ordering::AcqRel);
                (
                    StatusCode::OK,
                    Bytes::from_static(
                        br#"{"config":{"currentPeriod":{"type":"WEEKLY","start":"2030-01-01T00:00:00Z","end":"2030-01-08T00:00:00Z"},"creditUsagePercent":37.5}}"#,
                    ),
                )
            }
            "/backend-api/wham/rate-limit-reset-credits" => (
                StatusCode::OK,
                Bytes::from(
                    serde_json::json!({
                        "available_count": self.available_count.load(Ordering::Acquire),
                        "credits": [{
                            "reset_type": "codex_rate_limits",
                            "status": "available",
                            "expires_at": "2026-07-25T00:00:00Z"
                        }]
                    })
                    .to_string(),
                ),
            ),
            "/backend-api/wham/rate-limit-reset-credits/consume" => {
                if self.block_consume {
                    self.consume_started.add_permits(1);
                    self.consume_release
                        .acquire()
                        .await
                        .expect("consume release signal")
                        .forget();
                }
                self.consume_calls.fetch_add(1, Ordering::AcqRel);
                self.available_count
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                        count.checked_sub(1)
                    })
                    .expect("consume requires an available credit");
                (
                    StatusCode::OK,
                    Bytes::from_static(br#"{"code":"ok","windows_reset":2}"#),
                )
            }
            other => panic!("unexpected quota request path: {other}"),
        };
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
