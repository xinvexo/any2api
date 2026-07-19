use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyAddress,
    ProxyDraft, ProxyKind, ProxyProfileId,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::{TempDir, tempdir};

use crate::{
    config_publish_error::ConfigPublishError,
    provider_api_key_secret::ProviderApiKeySecret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn commit_reconcile_and_snapshot_switch_share_one_revision() {
    let context = TestContext::new().await;
    let id = ProxyProfileId::new();

    let published = context
        .publisher
        .create_proxy(ConfigRevision::INITIAL, id, proxy_draft("Hong Kong"))
        .await
        .expect("publish proxy");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision().get(), 2);
    assert_eq!(published.revision(), stored.revision());
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert!(published.proxies().get(id).is_some());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn stale_publish_is_rejected_before_storage_changes() {
    let context = TestContext::new().await;
    let first_id = ProxyProfileId::new();
    let current = context
        .publisher
        .create_proxy(ConfigRevision::INITIAL, first_id, proxy_draft("Hong Kong"))
        .await
        .expect("first publish");
    let second_id = ProxyProfileId::new();

    let error = context
        .publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            second_id,
            proxy_draft("United States"),
        )
        .await
        .expect_err("stale publish must fail");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert!(matches!(error, ConfigPublishError::RevisionConflict { .. }));
    assert_eq!(stored.revision(), current.revision());
    assert!(stored.proxies().get(second_id).is_none());
    assert_eq!(context.snapshots.load().revision(), current.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn no_op_publish_keeps_revision_and_scheduler_epoch() {
    let context = TestContext::new().await;

    let published = context
        .publisher
        .set_global_proxy(ConfigRevision::INITIAL, ProxyProfileId::DIRECT)
        .await
        .expect("no-op publish");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision(), ConfigRevision::INITIAL);
    assert_eq!(stored.revision(), ConfigRevision::INITIAL);
    assert_eq!(context.runtime.scheduler_epoch(), 0);
}

#[tokio::test]
async fn provider_endpoint_publish_switches_the_complete_snapshot() {
    let context = TestContext::new().await;
    let id = ProviderEndpointId::new();
    let published = context
        .publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            id,
            ProviderEndpointDraft::new(
                "Codex Primary",
                ProviderKind::Codex,
                "https://api.example.com/v1/",
                ProtocolDialect::OpenAiResponses,
                false,
                false,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("publish endpoint");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision(), stored.revision());
    assert!(published.provider_endpoints().get(id).is_some());
    assert!(published.proxies().profiles().len() == stored.proxies().profiles().len());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn provider_credential_publish_switches_the_complete_snapshot() {
    let context = TestContext::new().await;
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = context
        .publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, codex_endpoint_draft())
        .await
        .expect("publish endpoint");

    let published = context
        .publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            ProviderApiKeySecret::new("sk-runtime-credential".to_owned()),
        )
        .await
        .expect("publish credential");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision(), stored.revision());
    assert!(
        published
            .provider_credentials()
            .get(credential_id)
            .is_some()
    );
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 2);
}

#[tokio::test]
async fn direct_credential_binding_resolves_the_published_global_proxy() {
    let context = TestContext::new().await;
    let proxy_id = ProxyProfileId::new();
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let proxy = context
        .publisher
        .create_proxy(ConfigRevision::INITIAL, proxy_id, proxy_draft("Hong Kong"))
        .await
        .expect("publish proxy");
    let global = context
        .publisher
        .set_global_proxy(proxy.revision(), proxy_id)
        .await
        .expect("publish global proxy");
    let endpoint = context
        .publisher
        .create_provider_endpoint(global.revision(), endpoint_id, codex_endpoint_draft())
        .await
        .expect("publish endpoint");
    let credential = context
        .publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            ProviderApiKeySecret::new("sk-runtime-proxy".to_owned()),
        )
        .await
        .expect("publish credential");

    assert_eq!(
        credential
            .resolved_proxy_for_credential(credential_id)
            .map(|profile| profile.id()),
        Some(proxy_id)
    );
}

#[tokio::test]
async fn publishers_sharing_a_snapshot_store_are_serialized() {
    let context = TestContext::new().await;
    let second_publisher = ConfigPublisher::new(
        Arc::clone(&context.repository),
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
    );
    let first_id = ProxyProfileId::new();
    let second_id = ProxyProfileId::new();

    let (first, second) = tokio::join!(
        context
            .publisher
            .create_proxy(ConfigRevision::INITIAL, first_id, proxy_draft("First")),
        second_publisher.create_proxy(ConfigRevision::INITIAL, second_id, proxy_draft("Second"))
    );
    let success_count = usize::from(first.is_ok()) + usize::from(second.is_ok());
    let conflict_count = usize::from(matches!(
        first,
        Err(ConfigPublishError::RevisionConflict { .. })
    )) + usize::from(matches!(
        second,
        Err(ConfigPublishError::RevisionConflict { .. })
    ));
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(success_count, 1);
    assert_eq!(conflict_count, 1);
    assert_eq!(stored.revision().get(), 2);
    assert_eq!(stored.proxies().profiles().len(), 2);
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

struct TestContext {
    _directory: TempDir,
    repository: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: ConfigPublisher,
}

impl TestContext {
    async fn new() -> Self {
        let directory = tempdir().expect("temporary directory");
        let repository = Arc::new(
            SqliteStore::connect(&directory.path().join("config.sqlite3"))
                .await
                .expect("repository"),
        );
        let initial = repository
            .load_configuration()
            .await
            .expect("initial configuration");
        let runtime = Arc::new(RuntimeRegistry::new(initial.settings().scheduler()));
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            initial,
            runtime.as_ref(),
        )));
        let publisher = ConfigPublisher::new(
            Arc::clone(&repository),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
        );

        Self {
            _directory: directory,
            repository,
            snapshots,
            runtime,
            publisher,
        }
    }
}

fn proxy_draft(name: &str) -> ProxyDraft {
    let address = ProxyAddress::new("proxy.example.com", 1080).expect("address");
    ProxyDraft::new(name, ProxyKind::Socks5, address, true).expect("draft")
}

fn codex_endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com",
        ProtocolDialect::OpenAiResponses,
        false,
        false,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(4).expect("max concurrency"),
        true,
    )
    .expect("credential draft")
}
