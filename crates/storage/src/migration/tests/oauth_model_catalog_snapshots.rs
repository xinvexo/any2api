use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn catalog_snapshot_migration_preserves_representative_oauth_and_quota_data() {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection");
    migrate_through(&mut connection, 33).await;
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, requests_per_minute, enabled) \
         VALUES ('11111111-1111-4111-8111-111111111111', 'codex', 'Existing', 'existing', \
                 CAST('{}' AS BLOB), 1, 1, 1, NULL, 1)",
    )
    .execute(&mut connection)
    .await
    .expect("OAuth account");
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload) \
         VALUES ('11111111-1111-4111-8111-111111111111', 9, 100, CAST('{}' AS BLOB))",
    )
    .execute(&mut connection)
    .await
    .expect("quota snapshot");

    migrate_through(&mut connection, 34).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=34).collect::<Vec<_>>()
    );
    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_accounts")
        .fetch_one(&mut connection)
        .await
        .expect("OAuth accounts");
    let quota_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_quota_snapshots")
        .fetch_one(&mut connection)
        .await
        .expect("quota snapshots");
    assert_eq!((account_count, quota_count), (1, 1));
    sqlx::query(
        "INSERT INTO oauth_model_catalog_snapshots \
         (provider_kind, directory_scope, fetched_at, models_json) \
         VALUES ('codex', 'free', 200, CAST('[\"gpt-example\"]' AS BLOB))",
    )
    .execute(&mut connection)
    .await
    .expect("catalog snapshot");
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
