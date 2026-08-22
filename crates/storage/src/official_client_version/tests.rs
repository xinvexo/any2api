use any2api_domain::ProviderKind;
use tempfile::tempdir;

use super::{OfficialClientVersionRepository, StoredOfficialClientVersion};
use crate::{error::StorageError, sqlite::SqliteStore};

#[tokio::test]
async fn versions_round_trip_and_upsert_by_provider() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("versions.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let mut codex = StoredOfficialClientVersion {
        provider_kind: ProviderKind::Codex,
        version: "1.2.3".into(),
        fetched_at: 100,
    };
    store
        .upsert_official_client_version(&codex)
        .await
        .expect("first version");
    codex.version = "1.2.4".into();
    codex.fetched_at = 200;
    store
        .upsert_official_client_version(&codex)
        .await
        .expect("updated version");
    let grok = StoredOfficialClientVersion {
        provider_kind: ProviderKind::Grok,
        version: "2.0.0".into(),
        fetched_at: 150,
    };
    store
        .upsert_official_client_version(&grok)
        .await
        .expect("second provider");
    drop(store);

    let reopened = SqliteStore::connect(&database)
        .await
        .expect("reopened store");
    assert_eq!(
        reopened
            .load_official_client_versions()
            .await
            .expect("versions"),
        vec![codex, grok]
    );
}

#[tokio::test]
async fn repository_rejects_non_stable_or_noncanonical_versions() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("versions.sqlite3"))
        .await
        .expect("store");
    for value in ["v1.2.3", "1.2.3-alpha.1", "1.2"] {
        let version = StoredOfficialClientVersion {
            provider_kind: ProviderKind::Claude,
            version: value.into(),
            fetched_at: 1,
        };
        assert!(matches!(
            store.upsert_official_client_version(&version).await,
            Err(StorageError::CorruptOfficialClientVersion)
        ));
    }
}
