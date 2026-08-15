use tempfile::tempdir;

use super::{
    MAX_OAUTH_MODEL_CATALOG_MODELS, OAuthModelCatalogSnapshotRepository,
    StoredOAuthModelCatalogSnapshot,
};
use crate::{error::StorageError, sqlite::SqliteStore};

#[tokio::test]
async fn shared_catalog_snapshot_round_trips_without_an_account_foreign_key() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("catalog.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let snapshot = StoredOAuthModelCatalogSnapshot {
        provider_kind: any2api_domain::ProviderKind::Codex,
        directory_scope: "plus_or_pro".into(),
        fetched_at: 100,
        models: vec!["gpt-a".into(), "gpt-z".into()],
    };
    store
        .upsert_oauth_model_catalog_snapshot(&snapshot)
        .await
        .expect("snapshot");
    drop(store);

    let reopened = SqliteStore::connect(&database)
        .await
        .expect("reopened store");
    assert_eq!(
        reopened
            .load_oauth_model_catalog_snapshots()
            .await
            .expect("snapshots"),
        vec![snapshot]
    );
}

#[tokio::test]
async fn repository_rejects_unsorted_or_unsafe_catalogs() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("catalog.sqlite3"))
        .await
        .expect("store");
    for snapshot in [
        StoredOAuthModelCatalogSnapshot {
            provider_kind: any2api_domain::ProviderKind::Codex,
            directory_scope: "invalid-scope".into(),
            fetched_at: 1,
            models: vec![],
        },
        StoredOAuthModelCatalogSnapshot {
            provider_kind: any2api_domain::ProviderKind::Claude,
            directory_scope: "subscription".into(),
            fetched_at: 1,
            models: vec!["model-z".into(), "model-a".into()],
        },
        StoredOAuthModelCatalogSnapshot {
            provider_kind: any2api_domain::ProviderKind::Grok,
            directory_scope: "subscription".into(),
            fetched_at: 1,
            models: vec!["model".into(); MAX_OAUTH_MODEL_CATALOG_MODELS + 1],
        },
    ] {
        assert!(matches!(
            store.upsert_oauth_model_catalog_snapshot(&snapshot).await,
            Err(StorageError::CorruptOAuthModelCatalogSnapshot)
        ));
    }
}
