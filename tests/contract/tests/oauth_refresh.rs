use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use any2api_contract_tests::{build_configuration_capabilities, build_provider_registry};
use any2api_domain::{
    GatewayApiKeyId, OAuthAccountDraft, OAuthAccountId, ProtocolOperation, ProviderKind, RequestId,
    RequestsPerMinute,
};
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
    OpenAiResponsesAdapter, ResponsesToChatCompletionsBridge, api::ProtocolRegistry,
};
use any2api_runtime::api::{
    ConfigPublisher, OAuthService, PublicRequest, PublicRequestService, PublicResponseBody,
    PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, SqliteStore};
use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{
    HeaderMap, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use tempfile::tempdir;

#[tokio::test]
async fn oauth_refresh_worker_keeps_a_disabled_account_alive_and_stops_with_the_process() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("oauth-refresh.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let providers = build_provider_registry();
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(initial, runtime.as_ref(), providers.as_ref())
            .expect("initial snapshot"),
    ));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            build_configuration_capabilities(),
        )
        .expect("publisher"),
    );
    let transport = Arc::new(RefreshTransport::default());
    let oauth = OAuthService::new(
        Arc::clone(&providers),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
        Arc::clone(&publisher),
        Arc::clone(&storage),
        Arc::new(RequestTelemetry::disabled()),
    );
    let lifecycle = runtime.lifecycle();
    assert!(oauth.start_refresh_worker(&lifecycle));
    assert!(!oauth.start_refresh_worker(&lifecycle));
    tokio::task::yield_now().await;

    let account_id = OAuthAccountId::new();
    let activated = publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            OAuthAccountDraft::new("Codex OAuth", None, false).expect("OAuth draft"),
            Some("person@example.com".into()),
            Some(0),
            vec!["gpt-5.5".into()],
            oauth_document(),
        )
        .await
        .expect("activate OAuth account");
    assert_eq!(activated.revision().get(), 2);

    let refreshed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = snapshots.load();
            if snapshot
                .oauth_accounts()
                .get(account_id)
                .is_some_and(|account| account.token_version() == 2)
            {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("refresh worker should publish promptly");

    assert_eq!(refreshed.revision().get(), 3);
    let account = refreshed
        .oauth_accounts()
        .get(account_id)
        .expect("refreshed account");
    assert_eq!(account.token_version(), 2);
    assert_eq!(account.account_generation(), 1);
    assert_eq!(account.config_version(), 1);
    assert!(!account.enabled());
    assert_eq!(account.safe_account_email(), Some("person@example.com"));
    assert_eq!(account.models()[0].as_str(), "gpt-5.5");
    assert_eq!(transport.calls(), 1);
    let captured = transport.take();
    assert_eq!(captured.proxy_id, any2api_domain::ProxyProfileId::DIRECT);
    assert_eq!(captured.host.as_deref(), Some("auth.openai.com"));
    assert_eq!(captured.content_type.as_deref(), Some("application/json"));
    let body = serde_json::from_slice::<serde_json::Value>(&captured.body).expect("refresh JSON");
    assert_eq!(body["grant_type"], "refresh_token");
    assert_eq!(body["refresh_token"], "old-refresh");

    assert!(lifecycle.begin_draining());
    lifecycle.close_background_tasks();
    tokio::time::timeout(
        Duration::from_secs(1),
        lifecycle.wait_for_background_tasks(),
    )
    .await
    .expect("refresh worker should stop while draining");
    assert_eq!(lifecycle.background_task_count(), 0);
}

#[tokio::test]
async fn pending_oauth_401_refreshes_once_and_replans_with_the_new_generation() {
    let context = AuthenticationRetryContext::new(false).await;

    let response = context
        .service
        .execute(context.snapshots.load(), oauth_request())
        .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(
        context.transport.data_authorizations(),
        vec!["Bearer old-access", "Bearer new-access"]
    );
    let snapshot = context.snapshots.load();
    assert_eq!(snapshot.revision().get(), 3);
    assert_eq!(
        snapshot
            .oauth_accounts()
            .get(context.account_id)
            .expect("OAuth account")
            .token_version(),
        2
    );
    assert_eq!(
        snapshot
            .credential_runtime(context.account_id.into())
            .expect("OAuth runtime")
            .rate_snapshot()
            .requests_in_window(),
        2
    );
}

#[tokio::test]
async fn a_second_oauth_401_never_refreshes_or_sends_a_third_attempt() {
    let context = AuthenticationRetryContext::new(true).await;

    let response = context
        .service
        .execute(context.snapshots.load(), oauth_request())
        .await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    match response.body {
        PublicResponseBody::Buffered(body) => assert_eq!(
            body,
            Bytes::from_static(
                br#"{"error":{"type":"authentication_error","code":"authentication_error"}}"#,
            )
        ),
        PublicResponseBody::Streaming(_) => panic!("401 response must be buffered"),
    }
    assert_eq!(context.transport.refresh_calls(), 1);
    assert_eq!(
        context.transport.data_authorizations(),
        vec!["Bearer old-access", "Bearer new-access"]
    );
    assert_eq!(
        context
            .snapshots
            .load()
            .credential_runtime(context.account_id.into())
            .expect("OAuth runtime")
            .rate_snapshot()
            .requests_in_window(),
        2
    );
}

#[tokio::test]
async fn completed_oauth_use_triggers_one_coalesced_persistent_quota_refresh() {
    let context = AuthenticationRetryContext::new(false).await;
    let lifecycle = context.runtime.lifecycle();
    assert!(context.oauth.start_quota_worker(&lifecycle));

    let response = context
        .service
        .execute(context.snapshots.load(), oauth_request())
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(context.transport.quota_calls(), 0);

    let cached = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Some(cached) = context
                .oauth
                .cached_quota(context.account_id)
                .await
                .expect("cached quota")
            {
                break cached;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("activity quota refresh");
    assert_eq!(context.transport.quota_calls(), 2);
    assert_eq!(
        cached
            .usage
            .rate_limit
            .and_then(|limit| limit.windows.into_iter().next())
            .map(|window| window.used_percent),
        Some(42.0)
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(context.transport.quota_calls(), 2);

    assert!(lifecycle.begin_draining());
    lifecycle.close_background_tasks();
    lifecycle.wait_for_background_tasks().await;
}

fn oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        serde_json::to_vec(&serde_json::json!({
            "access_token": "old-access",
            "refresh_token": "old-refresh",
            "id_token": "old-id-token",
            "account_id": "account-123",
            "email": "person@example.com",
        }))
        .expect("current OAuth document")
        .into(),
    )
    .expect("OAuth document")
}

fn oauth_request() -> PublicRequest {
    PublicRequest {
        request_id: RequestId::new(),
        gateway_api_key_id: GatewayApiKeyId::new(),
        client_ip: "127.0.0.1".parse().expect("client IP"),
        operation: ProtocolOperation::Responses,
        headers: HeaderMap::new(),
        body: Bytes::from_static(br#"{"model":"gpt-5.5","input":"hello"}"#),
    }
}

struct AuthenticationRetryContext {
    _directory: tempfile::TempDir,
    _storage: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    oauth: Arc<OAuthService>,
    service: PublicRequestService,
    transport: Arc<AuthenticationRetryTransport>,
    account_id: OAuthAccountId,
}

impl AuthenticationRetryContext {
    async fn new(always_unauthorized: bool) -> Self {
        let directory = tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("oauth-auth-retry.sqlite3"))
                .await
                .expect("storage"),
        );
        let initial = storage.load_configuration().await.expect("configuration");
        let providers = build_provider_registry();
        let protocols = protocols();
        let runtime = Arc::new(RuntimeRegistry::new());
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(initial, runtime.as_ref(), providers.as_ref())
                .expect("initial snapshot"),
        ));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                build_configuration_capabilities(),
            )
            .expect("publisher"),
        );
        let transport = Arc::new(AuthenticationRetryTransport::new(always_unauthorized));
        let service = PublicRequestService::new(
            protocols,
            Arc::clone(&providers),
            Arc::clone(&transport) as Arc<dyn TransportManager>,
        )
        .expect("public request service");
        let oauth = Arc::new(OAuthService::new(
            providers,
            Arc::clone(&transport) as Arc<dyn TransportManager>,
            Arc::clone(&publisher),
            Arc::clone(&storage),
            Arc::new(RequestTelemetry::disabled()),
        ));
        assert!(service.install_oauth(oauth.as_ref()));
        let account_id = OAuthAccountId::new();
        publisher
            .activate_oauth_account(
                account_id,
                ProviderKind::Codex,
                OAuthAccountDraft::new(
                    "Codex OAuth",
                    Some(RequestsPerMinute::new(2).expect("valid RPM")),
                    true,
                )
                .expect("OAuth draft"),
                Some("person@example.com".into()),
                None,
                vec!["gpt-5.5".into()],
                oauth_document(),
            )
            .await
            .expect("activate OAuth account");
        Self {
            _directory: directory,
            _storage: storage,
            snapshots,
            runtime,
            oauth,
            service,
            transport,
            account_id,
        }
    }
}

fn protocols() -> Arc<ProtocolRegistry> {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    protocols
        .register(Arc::new(OpenAiChatCompletionsAdapter::new()))
        .expect("Chat Completions adapter");
    protocols
        .register(Arc::new(OpenAiImagesAdapter::new()))
        .expect("Images adapter");
    protocols
        .register(Arc::new(AnthropicMessagesAdapter::new()))
        .expect("Messages adapter");
    protocols
        .register_bridge(Arc::new(ResponsesToChatCompletionsBridge::new()))
        .expect("Responses bridge");
    Arc::new(protocols)
}

struct CapturedRefreshRequest {
    proxy_id: any2api_domain::ProxyProfileId,
    host: Option<String>,
    content_type: Option<String>,
    body: Bytes,
}

#[derive(Default)]
struct RefreshTransport {
    calls: AtomicUsize,
    captured: Mutex<Option<CapturedRefreshRequest>>,
}

impl RefreshTransport {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }

    fn take(&self) -> CapturedRefreshRequest {
        self.captured
            .lock()
            .expect("captured request lock")
            .take()
            .expect("captured refresh request")
    }
}

#[async_trait]
impl TransportManager for RefreshTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        *self.captured.lock().expect("captured request lock") = Some(CapturedRefreshRequest {
            proxy_id: proxy.profile().id(),
            host: request.uri.host().map(str::to_owned),
            content_type: request
                .headers
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            body: request.body,
        });
        let body: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
            br#"{"access_token":"new-access","expires_in":3600}"#,
        ))]));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

struct AuthenticationRetryTransport {
    always_unauthorized: bool,
    refresh_calls: AtomicUsize,
    quota_calls: AtomicUsize,
    data_authorizations: Mutex<Vec<String>>,
}

impl AuthenticationRetryTransport {
    fn new(always_unauthorized: bool) -> Self {
        Self {
            always_unauthorized,
            refresh_calls: AtomicUsize::new(0),
            quota_calls: AtomicUsize::new(0),
            data_authorizations: Mutex::new(Vec::new()),
        }
    }

    fn refresh_calls(&self) -> usize {
        self.refresh_calls.load(Ordering::Acquire)
    }

    fn quota_calls(&self) -> usize {
        self.quota_calls.load(Ordering::Acquire)
    }

    fn data_authorizations(&self) -> Vec<String> {
        self.data_authorizations
            .lock()
            .expect("authorization lock")
            .clone()
    }
}

#[async_trait]
impl TransportManager for AuthenticationRetryTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), any2api_domain::ProxyProfileId::DIRECT);
        let host = request.uri.host().expect("request host");
        let path = request.uri.path();
        if path == "/backend-api/wham/usage" || path == "/backend-api/wham/rate-limit-reset-credits"
        {
            self.quota_calls.fetch_add(1, Ordering::AcqRel);
            let body = if path == "/backend-api/wham/usage" {
                Bytes::from_static(
                    br#"{"rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":42.0,"limit_window_seconds":18000,"reset_after_seconds":300,"reset_at":1900000000},"secondary_window":null}}"#,
                )
            } else {
                Bytes::from_static(br#"{"available_count":0,"credits":[]}"#)
            };
            return Ok(TransportResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: Box::pin(stream::iter([Ok(body)])),
                read_failure_scope: TransportFailureScope::Endpoint,
            });
        }
        let (status, body) = if host == "auth.openai.com" {
            self.refresh_calls.fetch_add(1, Ordering::AcqRel);
            assert_eq!(request.headers[CONTENT_TYPE], "application/json");
            let body =
                serde_json::from_slice::<serde_json::Value>(&request.body).expect("refresh JSON");
            assert_eq!(body["refresh_token"], "old-refresh");
            (
                StatusCode::OK,
                Bytes::from_static(
                    br#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#,
                ),
            )
        } else {
            assert_eq!(host, "chatgpt.com");
            let authorization = request
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .expect("OAuth authorization")
                .to_owned();
            let mut authorizations = self.data_authorizations.lock().expect("authorization lock");
            authorizations.push(authorization);
            let attempt = authorizations.len();
            drop(authorizations);
            if self.always_unauthorized || attempt == 1 {
                (
                    StatusCode::UNAUTHORIZED,
                    Bytes::from_static(
                        br#"{"error":{"type":"authentication_error","code":"authentication_error"}}"#,
                    ),
                )
            } else {
                (
                    StatusCode::OK,
                    Bytes::from_static(
                        br#"{"id":"resp_refreshed","object":"response","status":"completed","model":"gpt-5.5","output":[]}"#,
                    ),
                )
            }
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
