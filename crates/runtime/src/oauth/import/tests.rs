use std::sync::Arc;

use any2api_domain::{ConfigRevision, ProviderKind};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use bytes::Bytes;
use serde_json::json;
use tempfile::{TempDir, tempdir};

use crate::{
    configuration::{ConfigPublisher, ConfigurationCapabilities, PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

use super::{OAuthImportError, OAuthImportFailureKind, publish};

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
    let projected_ids = snapshot
        .routing_credentials()
        .iter()
        .map(|credential| credential.id())
        .collect::<Vec<_>>();
    let runtime_ids = snapshot
        .credential_runtimes()
        .map(|binding| binding.credential_id())
        .collect::<Vec<_>>();
    assert_eq!(runtime_ids, projected_ids);
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
async fn batch_import_trims_at_the_label_limit_before_deduplicating() {
    let context = ImportContext::new().await;
    let preferred = format!("{} tail", "a".repeat(99));
    let document = json!({
        "accounts": [
            {
                "name": preferred,
                "platform": "openai",
                "type": "oauth",
                "credentials": {"access_token": "codex-one"}
            },
            {
                "name": preferred,
                "platform": "openai",
                "type": "oauth",
                "credentials": {"access_token": "codex-two"}
            }
        ]
    });

    let result = publish(
        context.capabilities.provider_registry(),
        &context.publisher,
        vec![json_bytes(document)],
    )
    .await
    .expect("batch import");
    let labels = result
        .accounts()
        .iter()
        .map(|account| account.label())
        .collect::<Vec<_>>();

    assert_eq!(labels, ["a".repeat(99), format!("{} (2)", "a".repeat(96))]);
    assert!(labels.iter().all(|label| label.trim_end() == *label));
    assert!(labels.iter().all(|label| label.chars().count() <= 100));
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

#[tokio::test]
async fn duplicate_import_identities_fail_the_whole_batch() {
    let context = ImportContext::new().await;
    let error = publish(
        context.capabilities.provider_registry(),
        &context.publisher,
        vec![json_bytes(json!({
            "accounts": [
                {
                    "platform": "openai",
                    "type": "oauth",
                    "credentials": {
                        "access_token": "first-access",
                        "email": "same@example.com"
                    }
                },
                {
                    "platform": "openai",
                    "type": "oauth",
                    "credentials": {
                        "access_token": "second-access",
                        "email": "SAME@example.com"
                    }
                }
            ]
        }))],
    )
    .await
    .expect_err("duplicate stable identity");

    assert!(matches!(
        error,
        OAuthImportError::Activation(
            crate::configuration::ConfigPublishError::OAuthAccountIdentityConflict
        )
    ));
    assert_initial(&context).await;
}

#[tokio::test]
async fn duplicate_token_against_an_existing_account_is_rejected() {
    let context = ImportContext::new().await;
    publish(
        context.capabilities.provider_registry(),
        &context.publisher,
        vec![Bytes::from_static(
            br#"{"type":"claude","access_token":"same-access"}"#,
        )],
    )
    .await
    .expect("first import");
    let before = context.snapshots.load().revision();

    let error = publish(
        context.capabilities.provider_registry(),
        &context.publisher,
        vec![Bytes::from_static(
            br#"{"type":"claude","access_token":"same-access"}"#,
        )],
    )
    .await
    .expect_err("duplicate exact token");

    assert!(matches!(
        error,
        OAuthImportError::Activation(
            crate::configuration::ConfigPublishError::OAuthAccountIdentityConflict
        )
    ));
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision(), before);
    assert_eq!(stored.oauth_accounts().accounts().len(), 1);
    assert_eq!(context.snapshots.load().revision(), before);
    assert_eq!(context.runtime.scheduler_epoch(), 1);
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
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(initial, runtime.as_ref(), capabilities.provider_registry())
                .expect("initial snapshot"),
        ));
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

async fn assert_initial(context: &ImportContext) {
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
