use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use any2api_domain::{ProviderKind, RetrySafety};
use any2api_provider::{ClaudeDriver, CodexDriver, GrokDriver, api::ProviderRegistry};
use any2api_storage::api::{
    ConfigurationRepository, OfficialClientVersionRepository, SqliteStore,
    StoredOfficialClientVersion,
};
use any2api_transport::api::{
    TransportError, TransportErrorStage, TransportFailureScope, TransportManager, TransportProxy,
    TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use tempfile::tempdir;
use tokio::sync::Barrier;

use super::{OfficialClientVersionService, unix_timestamp};
use crate::{
    configuration::{PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn initialize_fetches_missing_versions_concurrently_then_persists_and_publishes() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("versions.sqlite3"))
            .await
            .expect("storage"),
    );
    let providers = versioned_providers(&[
        ProviderKind::Codex,
        ProviderKind::Claude,
        ProviderKind::Grok,
    ]);
    let transport = Arc::new(ConcurrentVersionTransport::new(3));
    let service = build_service(
        Arc::clone(&providers),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
        Arc::clone(&storage),
    )
    .await;

    tokio::time::timeout(std::time::Duration::from_secs(2), service.initialize())
        .await
        .expect("version sources are fetched concurrently")
        .expect("initialization");

    assert_eq!(transport.calls.load(Ordering::Acquire), 3);
    assert_current_version(&providers, ProviderKind::Codex, "9.8.7");
    assert_current_version(&providers, ProviderKind::Claude, "8.7.6");
    assert_current_version(&providers, ProviderKind::Grok, "7.6.5");

    let rows = storage
        .load_official_client_versions()
        .await
        .expect("persisted versions");
    assert_eq!(rows.len(), 3);
    for (provider, expected) in [
        (ProviderKind::Codex, "9.8.7"),
        (ProviderKind::Claude, "8.7.6"),
        (ProviderKind::Grok, "7.6.5"),
    ] {
        let row = rows
            .iter()
            .find(|row| row.provider_kind == provider)
            .expect("provider row");
        assert_eq!(row.version, expected);
        assert!(row.fetched_at > 0);
    }
}

#[tokio::test]
async fn initialize_loads_last_known_good_and_retains_it_when_refresh_fails() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("versions.sqlite3"))
            .await
            .expect("storage"),
    );
    let stored = StoredOfficialClientVersion {
        provider_kind: ProviderKind::Codex,
        version: "1.2.3".into(),
        fetched_at: unix_timestamp().saturating_sub(super::SOURCE_REFRESH_INTERVAL_SECS),
    };
    storage
        .upsert_official_client_version(&stored)
        .await
        .expect("stored version");
    let providers = versioned_providers(&[ProviderKind::Codex]);
    let transport = Arc::new(FailingVersionTransport::default());
    let service = build_service(
        Arc::clone(&providers),
        Arc::clone(&transport) as Arc<dyn TransportManager>,
        Arc::clone(&storage),
    )
    .await;

    service.initialize().await.expect("initialization");

    assert_eq!(transport.calls.load(Ordering::Acquire), 1);
    assert_current_version(&providers, ProviderKind::Codex, "1.2.3");
    assert_eq!(
        storage
            .load_official_client_versions()
            .await
            .expect("persisted version"),
        vec![stored]
    );
}

fn versioned_providers(kinds: &[ProviderKind]) -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    for kind in kinds {
        let driver: Arc<dyn any2api_provider::api::ProviderDriver> = match kind {
            ProviderKind::Codex => Arc::new(CodexDriver::new()),
            ProviderKind::Claude => Arc::new(ClaudeDriver::new()),
            ProviderKind::Grok => Arc::new(GrokDriver::new()),
            _ => panic!("test provider does not expose an official client version"),
        };
        providers.register(driver).expect("provider registration");
    }
    Arc::new(providers)
}

async fn build_service(
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    storage: Arc<SqliteStore>,
) -> Arc<OfficialClientVersionService> {
    let configuration = storage.load_configuration().await.expect("configuration");
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(configuration, &RuntimeRegistry::new(), providers.as_ref())
            .expect("snapshot"),
    ));
    OfficialClientVersionService::new(providers, transport, snapshots, storage)
}

fn assert_current_version(providers: &ProviderRegistry, provider: ProviderKind, expected: &str) {
    let actual = providers
        .get(provider)
        .and_then(|driver| driver.official_client_version())
        .and_then(|versioned| versioned.current_official_client_version())
        .expect("current official client version");
    assert_eq!(actual.as_str(), expected);
}

struct ConcurrentVersionTransport {
    barrier: Barrier,
    calls: AtomicUsize,
}

impl ConcurrentVersionTransport {
    fn new(source_count: usize) -> Self {
        Self {
            barrier: Barrier::new(source_count),
            calls: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl TransportManager for ConcurrentVersionTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.barrier.wait().await;
        let body = match request.uri.host().expect("source host") {
            "releases.openai.com" => Bytes::from_static(br#"{"tag_name":"rust-v9.8.7"}"#),
            "downloads.claude.ai" => Bytes::from_static(b"8.7.6\n"),
            "x.ai" => Bytes::from_static(b"7.6.5\n"),
            host => panic!("unexpected source host: {host}"),
        };
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async move { Ok(body) })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

#[derive(Default)]
struct FailingVersionTransport {
    calls: AtomicUsize,
}

#[async_trait]
impl TransportManager for FailingVersionTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        _request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        Err(TransportError::new(
            TransportErrorStage::AwaitHeaders,
            TransportFailureScope::Endpoint,
            RetrySafety::DefinitelyNotSent,
            "version source unavailable",
        ))
    }
}
