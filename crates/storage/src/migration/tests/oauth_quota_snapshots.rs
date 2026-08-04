use sqlx::{Connection, SqliteConnection};

use super::{
    foreign_key_violations, migrate_through, migration_versions, table_schema_on_connection,
};

const ACCOUNT_ID: &str = "11111111-1111-4111-8111-111111111111";

#[tokio::test]
async fn quota_snapshot_migration_preserves_accounts_and_adds_bounded_cascade_table() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 11).await;
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, requests_per_minute, enabled) \
         VALUES (?, 'claude', 'Existing account', 'existing account', \
                 CAST('{\"access_token\":\"preserved\"}' AS BLOB), 3, 4, 5, 60, 1)",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut connection)
    .await
    .expect("representative OAuth account");

    migrate_through(&mut connection, 12).await;

    let preserved = sqlx::query_as::<_, (Vec<u8>, i64, i64, i64)>(
        "SELECT oauth_json, token_version, account_generation, config_version \
         FROM oauth_accounts WHERE id = ?",
    )
    .bind(ACCOUNT_ID)
    .fetch_one(&mut connection)
    .await
    .expect("preserved account");
    assert_eq!(
        preserved,
        (br#"{"access_token":"preserved"}"#.to_vec(), 3, 4, 5)
    );
    let schema = table_schema_on_connection(&mut connection, "oauth_quota_snapshots").await;
    assert!(schema.contains("schema_version = 1"));
    assert!(schema.contains("length(payload) BETWEEN 2 AND 262144"));
    assert!(schema.contains("ON DELETE CASCADE"));

    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload) \
         VALUES (?, 1, 1234, CAST('{}' AS BLOB))",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut connection)
    .await
    .expect("quota snapshot");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut connection)
        .await
        .expect("foreign keys");
    sqlx::query("DELETE FROM oauth_accounts WHERE id = ?")
        .bind(ACCOUNT_ID)
        .execute(&mut connection)
        .await
        .expect("delete account");
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM oauth_quota_snapshots")
        .fetch_one(&mut connection)
        .await
        .expect("remaining snapshots");
    assert_eq!(remaining, 0);
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=12).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
