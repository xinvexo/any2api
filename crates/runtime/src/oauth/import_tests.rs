use std::sync::Arc;

use any2api_domain::{ConfigRevision, ProviderKind};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use bytes::Bytes;
use serde_json::json;
use tempfile::{TempDir, tempdir};

use crate::{
    configuration_capabilities::ConfigurationCapabilities,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
};

use super::{OAuthImportError, OAuthImportFailureKind, import::publish};

#[tokio::test]
async fn batch_import_publishes_one_revision_and_compiles_routing_credentials() {
    let context = ImportContext::new().await;
    let first = json!({
        "accounts": [
            {
                "name": "Shared",
                "platform": "openai",
                "type": "oauth",
                "credentials": {"access_token": "codex-one"}
            },
            {
                "name": "Claude Imported",
                "platform": "anthropic",
                "type": "oauth",
                "credentials": {"access_token": "claude-one"}
            }
        ]
    });
    let second = json!({
        "name": "Shared",
        "type": "codex",
        "access_token": "codex-two"
    });

    let result = publish(
        context.capabilities.provider_registry(),
        &context.publisher,
        vec![json_bytes(first), json_bytes(second)],
    )
    .await
    .expect("batch import");
    let snapshot = context.snapshots.load();

    assert_eq!(result.revision().get(), 2);
    assert_eq!(result.accounts().len(), 3);
    assert_eq!(snapshot.oauth_accounts().accounts().len(), 3);
    assert_eq!(snapshot.credential_runtimes().len(), 3);
    let codex_labels = snapshot
        .oauth_accounts()
        .for_provider(ProviderKind::Codex)
        .map(|account| account.label())
        .collect::<Vec<_>>();
    assert_eq!(codex_labels, ["Shared", "Shared (2)"]);
    assert!(
        snapshot
            .oauth_accounts()
            .accounts()
            .iter()
            .all(|account| account.enabled() && account.requests_per_minute().is_none())
    );
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn invalid_later_file_leaves_sqlite_and_snapshot_unchanged() {
    let context = ImportContext::new().await;
    let error = publish(
        context.capabilities.provider_registry(),
        &context.publisher,
        vec![
            Bytes::from_static(br#"{"type":"codex","access_token":"valid"}"#),
            Bytes::from_static(br#"{"type":"codex","access_token":""}"#),
        ],
    )
    .await
    .expect_err("invalid second file");

    assert!(matches!(
        error,
        OAuthImportError::InvalidFile {
            file_index: 2,
            account_index: Some(1),
            kind: OAuthImportFailureKind::InvalidAccount,
        }
    ));
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision(), ConfigRevision::INITIAL);
    assert!(stored.oauth_accounts().accounts().is_empty());
    assert_eq!(context.snapshots.load().revision(), ConfigRevision::INITIAL);
    assert_eq!(context.runtime.scheduler_epoch(), 0);
}

struct ImportContext {
    _directory: TempDir,
    repository: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    capabilities: Arc<ConfigurationCapabilities>,
    publisher: ConfigPublisher,
}

impl ImportContext {
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
        let runtime = Arc::new(RuntimeRegistry::new());
        let capabilities = crate::test_support::configuration_capabilities();
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            initial,
            runtime.as_ref(),
            capabilities.provider_registry(),
        )));
        let publisher = ConfigPublisher::new(
            Arc::clone(&repository),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            Arc::clone(&capabilities),
        )
        .expect("publisher");
        Self {
            _directory: directory,
            repository,
            snapshots,
            runtime,
            capabilities,
            publisher,
        }
    }
}

fn json_bytes(value: serde_json::Value) -> Bytes {
    Bytes::from(serde_json::to_vec(&value).expect("JSON"))
}
