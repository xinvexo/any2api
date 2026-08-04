use any2api_domain::OAuthAccountId;
use tempfile::{TempDir, tempdir};

use super::{
    MAX_OAUTH_QUOTA_SNAPSHOT_BYTES, OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
    OAuthQuotaSnapshotRepository, StoredOAuthQuotaSnapshot,
};
use crate::{error::StorageError, sqlite::SqliteStore};

#[tokio::test]
async fn snapshot_round_trip_survives_reconnect_is_monotonic_and_cascades() {
    let (directory, store, id) = store_with_account().await;
    let current = snapshot(id, 200, br#"{"value":2}"#.to_vec());
    store
        .upsert_oauth_quota_snapshot(&current)
        .await
        .expect("current snapshot");
    store
        .upsert_oauth_quota_snapshot(&snapshot(id, 100, br#"{"value":1}"#.to_vec()))
        .await
        .expect("older snapshot is ignored");
    drop(store);
    let store = SqliteStore::connect(&directory.path().join("quota.sqlite3"))
        .await
        .expect("reopened store");
    assert_eq!(
        store
            .load_oauth_quota_snapshot(id)
            .await
            .expect("load snapshot"),
        Some(current)
    );

    sqlx::query("DELETE FROM oauth_accounts WHERE id = ?")
        .bind(id.to_string())
        .execute(store.pool())
        .await
        .expect("delete account");
    assert_eq!(
        store
            .load_oauth_quota_snapshot(id)
            .await
            .expect("load after cascade"),
        None
    );
}

#[tokio::test]
async fn repository_rejects_invalid_version_time_and_payload_size() {
    let (_directory, store, id) = store_with_account().await;
    for invalid in [
        StoredOAuthQuotaSnapshot {
            schema_version: 2,
            ..snapshot(id, 1, b"{}".to_vec())
        },
        snapshot(id, -1, b"{}".to_vec()),
        snapshot(id, 1, vec![b'x'; MAX_OAUTH_QUOTA_SNAPSHOT_BYTES + 1]),
    ] {
        assert!(matches!(
            store.upsert_oauth_quota_snapshot(&invalid).await,
            Err(StorageError::CorruptOAuthQuotaSnapshot)
        ));
    }
}

#[tokio::test]
async fn delete_reports_whether_a_snapshot_changed() {
    let (_directory, store, id) = store_with_account().await;
    assert!(
        !store
            .delete_oauth_quota_snapshot(id)
            .await
            .expect("empty delete")
    );
    store
        .upsert_oauth_quota_snapshot(&snapshot(id, 1, b"{}".to_vec()))
        .await
        .expect("snapshot");
    assert!(store.delete_oauth_quota_snapshot(id).await.expect("delete"));
    assert!(
        !store
            .delete_oauth_quota_snapshot(id)
            .await
            .expect("repeat delete")
    );
}

fn snapshot(
    oauth_account_id: OAuthAccountId,
    fetched_at: i64,
    payload: Vec<u8>,
) -> StoredOAuthQuotaSnapshot {
    StoredOAuthQuotaSnapshot {
        oauth_account_id,
        schema_version: OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
        fetched_at,
        payload,
    }
}

async fn store_with_account() -> (TempDir, SqliteStore, OAuthAccountId) {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("quota.sqlite3"))
        .await
        .expect("store");
    let id = OAuthAccountId::new();
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, requests_per_minute, enabled) \
         VALUES (?, 'codex', 'Quota test', 'quota test', CAST('{}' AS BLOB), 1, 1, 1, NULL, 1)",
    )
    .bind(id.to_string())
    .execute(store.pool())
    .await
    .expect("OAuth account");
    (directory, store, id)
}
