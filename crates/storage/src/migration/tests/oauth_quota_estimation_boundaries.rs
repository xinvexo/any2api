use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const ACCOUNT_ID: &str = "11111111-1111-4111-8111-111111111111";

#[tokio::test]
async fn estimation_boundary_migration_preserves_existing_quota_data_and_cascades() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 21).await;
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, requests_per_minute, enabled) \
         VALUES (?, 'codex', 'Existing account', 'existing account', \
                 CAST('{\"access_token\":\"preserved\"}' AS BLOB), 3, 4, 5, 60, 1)",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut connection)
    .await
    .expect("representative OAuth account");
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload) \
         VALUES (?, 2, 1234, CAST('{\"usage\":{},\"usd_estimates\":[]}' AS BLOB))",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut connection)
    .await
    .expect("representative quota snapshot");

    migrate_through(&mut connection, 22).await;

    let snapshot_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM oauth_quota_snapshots WHERE oauth_account_id = ?")
            .bind(ACCOUNT_ID)
            .fetch_one(&mut connection)
            .await
            .expect("preserved quota snapshot");
    assert_eq!(snapshot_count, 1);
    sqlx::query(
        "INSERT INTO oauth_quota_estimation_boundaries (oauth_account_id, reset_at_ms) \
         VALUES (?, 5678)",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut connection)
    .await
    .expect("estimation boundary");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut connection)
        .await
        .expect("foreign keys");
    sqlx::query("DELETE FROM oauth_accounts WHERE id = ?")
        .bind(ACCOUNT_ID)
        .execute(&mut connection)
        .await
        .expect("delete account");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM oauth_quota_estimation_boundaries")
            .fetch_one(&mut connection)
            .await
            .expect("remaining boundaries");
    assert_eq!(remaining, 0);
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=22).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
