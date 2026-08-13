mod cache;
mod filtering;
mod oauth;

use std::sync::Arc;

use any2api_domain::{
    CredentialKind, ProtocolDialect, ProviderCredentialDraft, ProviderEndpointDraft, ProviderKind,
    ProxyProfileId,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use tempfile::{TempDir, tempdir};

use crate::{
    configuration::{ConfigPublisher, ConfigurationCapabilities, PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

struct PublisherFixture {
    publisher: ConfigPublisher,
    capabilities: Arc<ConfigurationCapabilities>,
    _directory: TempDir,
}

async fn publisher_fixture() -> PublisherFixture {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let capabilities = crate::test_support::configuration_capabilities();
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(initial, runtime.as_ref(), capabilities.provider_registry())
            .expect("initial snapshot"),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        Arc::clone(&capabilities),
    )
    .expect("configuration publisher");
    PublisherFixture {
        publisher,
        capabilities,
        _directory: directory,
    }
}

fn endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com/v1",
        ProtocolDialect::OpenAiResponses,
        None,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft(label: &str) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        None,
        true,
    )
    .expect("credential draft")
}
