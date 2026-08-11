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

#[tokio::test]
async fn quota_snapshot_v2_migration_wraps_representative_usage_without_losing_observation() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 20).await;
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
    let usage = br#"{"rate_limit":{"allowed":true,"limit_reached":false,"windows":[]},"reset_credits":null,"billing":null,"token_balance":null,"subscription_tier":null,"account_status":null}"#;
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 1, 1234, CAST(? AS BLOB), '2026-08-11 00:00:00')",
    )
    .bind(ACCOUNT_ID)
    .bind(usage.as_slice())
    .execute(&mut connection)
    .await
    .expect("v1 quota snapshot");

    migrate_through(&mut connection, 21).await;

    let (version, fetched_at, payload, updated_at) =
        sqlx::query_as::<_, (i64, i64, Vec<u8>, String)>(
            "SELECT schema_version, fetched_at, payload, updated_at \
             FROM oauth_quota_snapshots WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .fetch_one(&mut connection)
        .await
        .expect("v2 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v2 payload JSON");
    assert_eq!(version, 2);
    assert_eq!(fetched_at, 1234);
    assert_eq!(updated_at, "2026-08-11 00:00:00");
    assert!(payload["usage"]["credits"].is_null());
    assert!(payload["usage"]["access"].is_null());
    assert_eq!(payload["usd_estimates"], serde_json::json!([]));
    let schema = table_schema_on_connection(&mut connection, "oauth_quota_snapshots").await;
    assert!(schema.contains("schema_version = 2"));
    assert!(schema.contains("length(payload) BETWEEN 2 AND 524288"));
    assert!(schema.contains("ON DELETE CASCADE"));
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=21).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

#[tokio::test]
async fn quota_snapshot_v3_migration_preserves_usage_and_discards_old_estimate_semantics() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 22).await;
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
    let payload = serde_json::json!({
        "usage": {
            "rate_limit": {"allowed": true, "limit_reached": false, "windows": []},
            "credits": null,
            "access": null,
            "reset_credits": null,
            "billing": null,
            "token_balance": null,
            "subscription_tier": null,
            "account_status": null
        },
        "usd_estimates": [{
            "window_id": "primary",
            "sample_used_percent_delta": 1.0
        }]
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 2, 1234, CAST(? AS BLOB), '2026-08-11 00:00:00')",
    )
    .bind(ACCOUNT_ID)
    .bind(serde_json::to_vec(&payload).expect("v2 payload"))
    .execute(&mut connection)
    .await
    .expect("v2 quota snapshot");

    migrate_through(&mut connection, 23).await;

    let (version, payload, updated_at) = sqlx::query_as::<_, (i64, Vec<u8>, String)>(
        "SELECT schema_version, payload, updated_at FROM oauth_quota_snapshots \
         WHERE oauth_account_id = ?",
    )
    .bind(ACCOUNT_ID)
    .fetch_one(&mut connection)
    .await
    .expect("v3 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v3 payload JSON");
    assert_eq!(version, 3);
    assert_eq!(payload["usage"]["rate_limit"]["allowed"], true);
    assert_eq!(payload["usd_estimates"], serde_json::json!([]));
    assert_eq!(updated_at, "2026-08-11 00:00:00");
    let schema = table_schema_on_connection(&mut connection, "oauth_quota_snapshots").await;
    assert!(schema.contains("schema_version = 3"));
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=23).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
