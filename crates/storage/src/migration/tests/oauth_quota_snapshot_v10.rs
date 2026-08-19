use sqlx::{Connection, SqliteConnection};

use super::{
    foreign_key_violations, migrate_through, migration_versions, table_schema_on_connection,
};

const ACCOUNT_ID: &str = "33333333-3333-4333-8333-333333333333";

#[tokio::test]
async fn quota_snapshot_v10_migration_preserves_usage_and_cold_starts_estimation() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 35).await;
    insert_representative_v9_snapshot(&mut connection).await;

    migrate_through(&mut connection, 36).await;

    let (version, fetched_at, payload, updated_at, storage_class) =
        sqlx::query_as::<_, (i64, i64, Vec<u8>, String, String)>(
            "SELECT schema_version, fetched_at, payload, updated_at, typeof(payload) \
             FROM oauth_quota_snapshots WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .fetch_one(&mut connection)
        .await
        .expect("v10 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v10 payload JSON");

    assert_eq!(version, 10);
    assert_eq!(fetched_at, 1_900_000_123);
    assert_eq!(updated_at, "2026-08-19 12:34:56");
    assert_eq!(storage_class, "blob");
    assert_eq!(payload["usage"], representative_usage());
    assert_eq!(payload["estimator_state"], serde_json::Value::Null);
    assert_eq!(payload.as_object().map(serde_json::Map::len), Some(2));
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=36).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());

    let schema = table_schema_on_connection(&mut connection, "oauth_quota_snapshots").await;
    assert!(schema.contains("schema_version = 10"));
    let quota_completion_index: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE type = 'index' \
         AND name = 'request_logs_oauth_quota_completion_idx'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("whole-cycle completion index");
    assert!(quota_completion_index.contains("started_at_ms + latency_ms"));
    assert!(
        sqlx::query(
            "UPDATE oauth_quota_snapshots SET schema_version = 9 WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .execute(&mut connection)
        .await
        .is_err()
    );
}

async fn insert_representative_v9_snapshot(connection: &mut SqliteConnection) {
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, requests_per_minute, enabled) \
         VALUES (?, 'codex', 'Existing account', 'existing account', \
                 CAST('{\"access_token\":\"preserved\"}' AS BLOB), 3, 4, 5, 60, 1)",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut *connection)
    .await
    .expect("representative OAuth account");
    let payload = serde_json::json!({
        "usage": representative_usage(),
        "estimator_state": {
            "credential_fingerprint": "identity-a",
            "subscription_tier": "pro",
            "windows": [{
                "key": {
                    "id": "five_hour",
                    "kind": "time",
                    "limit_window_seconds": 18_000
                },
                "anchor": {
                    "used_percent": 42.0,
                    "reset_at": 1_900_001_000,
                    "position": {
                        "process_id": "44444444-4444-4444-8444-444444444444",
                        "sequence": 11
                    }
                },
                "total_delta_used_percent": 10.0,
                "total_local_cost_credits": 156.0,
                "completed_interval_count": 2
            }]
        }
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 9, ?, CAST(? AS BLOB), ?)",
    )
    .bind(ACCOUNT_ID)
    .bind(1_900_000_123_i64)
    .bind(serde_json::to_vec(&payload).expect("v9 payload"))
    .bind("2026-08-19 12:34:56")
    .execute(connection)
    .await
    .expect("v9 quota snapshot");
}

fn representative_usage() -> serde_json::Value {
    serde_json::json!({
        "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "windows": [{
                "id": "five_hour",
                "kind": "time",
                "used_percent": 42.0,
                "limit_window_seconds": 18_000,
                "reset_after_seconds": 877,
                "reset_at": 1_900_001_000
            }]
        },
        "credits": {
            "balance": "12.5",
            "unlimited": false
        },
        "access": null,
        "reset_credits": null,
        "billing": null,
        "token_balance": null,
        "subscription_tier": "pro",
        "account_status": null
    })
}
