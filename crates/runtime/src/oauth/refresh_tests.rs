use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use any2api_domain::{MaxConcurrency, OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_provider::{CodexDriver, ProviderRegistry};
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
use http::{HeaderMap, StatusCode};
use tempfile::TempDir;
use tokio::sync::Semaphore;

use super::refresh::OAuthRefresher;
use crate::{
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn concurrent_refreshes_share_one_request_and_publish_one_generation() {
    let transport = Arc::new(BlockingRefreshTransport::new());
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");

    let first_refresher = Arc::clone(&context.refresher);
    let first = tokio::spawn(async move { first_refresher.refresh_if_due(id, 1).await });
    transport.wait_until_started().await;

    let second_refresher = Arc::clone(&context.refresher);
    let second = tokio::spawn(async move { second_refresher.refresh_if_due(id, 1).await });
    tokio::task::yield_now().await;
    assert_eq!(transport.calls(), 1);
    transport.release();

    assert_eq!(
        first.await.expect("first refresh").expect("first result"),
        Some(2)
    );
    assert_eq!(
        second
            .await
            .expect("second refresh")
            .expect("second result"),
        None
    );
    assert_eq!(transport.calls(), 1);
    let captured = transport.take_request();
    assert_eq!(captured.proxy_id, any2api_domain::ProxyProfileId::DIRECT);
    assert_eq!(captured.host.as_deref(), Some("auth.openai.com"));
    let form: std::collections::HashMap<_, _> = url::form_urlencoded::parse(&captured.body)
        .into_owned()
        .collect();
    assert_eq!(
        form.get("grant_type").map(String::as_str),
        Some("refresh_token")
    );
    assert_eq!(
        form.get("refresh_token").map(String::as_str),
        Some("old-refresh")
    );

    let published = context.snapshots.load();
    let account = published
        .oauth_accounts()
        .get(id)
        .expect("refreshed account");
    assert_eq!(account.token_version(), 2);
    assert_eq!(account.account_generation(), 2);
    assert_eq!(account.safe_account_email(), Some("person@example.com"));
    assert_eq!(account.models()[0].as_str(), "gpt-5.5");
    let token = published
        .oauth_token_material(id)
        .expect("published OAuth token");
    assert_eq!(token.access_token(), "new-access");
    assert_eq!(token.refresh_token(), Some("old-refresh"));
    assert_eq!(token.id_token(), Some("old-id-token"));
    assert_eq!(token.account_id(), Some("account-123"));
    assert_eq!(token.email(), Some("person@example.com"));
}

#[tokio::test]
async fn refresh_without_expiry_preserves_the_persisted_fail_closed_boundary() {
    let transport = Arc::new(BlockingRefreshTransport::with_response(Bytes::from_static(
        br#"{"access_token":"new-access"}"#,
    )));
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");
    transport.release();

    assert_eq!(
        context
            .refresher
            .refresh_if_due(id, 1)
            .await
            .expect("refresh result"),
        Some(2)
    );
    let published = context.snapshots.load();
    assert_eq!(
        published
            .oauth_accounts()
            .get(id)
            .expect("refreshed account")
            .expires_at(),
        Some(0)
    );
    assert_eq!(
        published
            .oauth_token_material(id)
            .expect("published OAuth token")
            .expires_at(),
        Some(0)
    );
}

#[tokio::test]
async fn concurrent_waiter_shares_a_failed_refresh_without_a_second_request() {
    let transport = Arc::new(BlockingRefreshTransport::with_status(
        StatusCode::BAD_REQUEST,
    ));
    let context = RefreshTestContext::with_account(Arc::clone(&transport)).await;
    let id = context.account_id.expect("OAuth account");

    let first_refresher = Arc::clone(&context.refresher);
    let first = tokio::spawn(async move { first_refresher.refresh_if_due(id, 1).await });
    transport.wait_until_started().await;
    let second_refresher = Arc::clone(&context.refresher);
    let second = tokio::spawn(async move { second_refresher.refresh_if_due(id, 1).await });
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    transport.release();

    assert!(first.await.expect("first refresh").is_err());
    assert_eq!(
        second
            .await
            .expect("second refresh")
            .expect("shared result"),
        None
    );
    assert_eq!(transport.calls(), 1);
}

struct RefreshTestContext {
    _directory: TempDir,
    _storage: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    _runtime: Arc<RuntimeRegistry>,
    refresher: Arc<OAuthRefresher>,
    account_id: Option<OAuthAccountId>,
}

impl RefreshTestContext {
    async fn with_account(transport: Arc<BlockingRefreshTransport>) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("oauth-refresh.sqlite3"))
                .await
                .expect("storage"),
        );
        let initial = storage.load_configuration().await.expect("configuration");
        let account_id = OAuthAccountId::new();
        let configured = storage
            .create_oauth_account(
                initial.revision(),
                account_id,
                ProviderKind::Codex,
                OAuthAccountDraft::new(
                    "Codex OAuth",
                    MaxConcurrency::new(1).expect("max concurrency"),
                    true,
                )
                .expect("OAuth draft"),
                Some("person@example.com".into()),
                Some(0),
                vec!["gpt-5.5".into()],
                oauth_document(),
            )
            .await
            .expect("OAuth account");
        let runtime = Arc::new(RuntimeRegistry::new(configured.settings().scheduler()));
        let capabilities = crate::test_support::configuration_capabilities();
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            configured,
            runtime.as_ref(),
            capabilities.provider_registry(),
        )));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                capabilities,
            )
            .expect("publisher"),
        );
        let providers = providers();
        let refresher =
            OAuthRefresher::new(providers, transport as Arc<dyn TransportManager>, publisher);
        Self {
            _directory: directory,
            _storage: storage,
            snapshots,
            _runtime: runtime,
            refresher,
            account_id: Some(account_id),
        }
    }
}

fn providers() -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    Arc::new(providers)
}

fn oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        br#"{"type":"codex","access_token":"old-access","refresh_token":"old-refresh","id_token":"old-id-token","account_id":"account-123","email":"person@example.com"}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}

struct CapturedRefreshRequest {
    proxy_id: any2api_domain::ProxyProfileId,
    host: Option<String>,
    body: Bytes,
}

struct BlockingRefreshTransport {
    started: Semaphore,
    release: Semaphore,
    calls: AtomicUsize,
    captured: Mutex<Option<CapturedRefreshRequest>>,
    response: Bytes,
    status: StatusCode,
}

impl BlockingRefreshTransport {
    fn new() -> Self {
        Self::with_response(Bytes::from_static(
            br#"{"access_token":"new-access","expires_in":3600}"#,
        ))
    }

    fn with_response(response: Bytes) -> Self {
        Self {
            started: Semaphore::new(0),
            release: Semaphore::new(0),
            calls: AtomicUsize::new(0),
            captured: Mutex::new(None),
            response,
            status: StatusCode::OK,
        }
    }

    fn with_status(status: StatusCode) -> Self {
        let mut transport = Self::new();
        transport.status = status;
        transport
    }

    async fn wait_until_started(&self) {
        self.started
            .acquire()
            .await
            .expect("refresh start signal")
            .forget();
    }

    fn release(&self) {
        self.release.add_permits(1);
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::Acquire)
    }

    fn take_request(&self) -> CapturedRefreshRequest {
        self.captured
            .lock()
            .expect("captured request lock")
            .take()
            .expect("captured refresh request")
    }
}

#[async_trait]
impl TransportManager for BlockingRefreshTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        *self.captured.lock().expect("captured request lock") = Some(CapturedRefreshRequest {
            proxy_id: proxy.profile().id(),
            host: request.uri.host().map(str::to_owned),
            body: request.body,
        });
        self.started.add_permits(1);
        self.release
            .acquire()
            .await
            .expect("refresh release signal")
            .forget();
        let body: BoxByteStream = Box::pin(stream::iter([Ok(self.response.clone())]));
        Ok(TransportResponse {
            status: self.status,
            headers: HeaderMap::new(),
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
