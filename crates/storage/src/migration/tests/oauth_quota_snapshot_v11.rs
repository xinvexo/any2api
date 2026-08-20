use serde_json::json;
use sqlx::{Connection, SqliteConnection};

use super::{
    foreign_key_violations, migrate_through, migration_versions, table_schema_on_connection,
};

const ACCOUNT_ID: &str = "33333333-3333-4333-8333-333333333333";

#[tokio::test]
async fn quota_snapshot_v11_preserves_usage_and_discards_mixed_cost_state() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 39).await;
    insert_mixed_v10_snapshot(&mut connection).await;

    migrate_through(&mut connection, 40).await;

    let (version, fetched_at, payload, updated_at, storage_class) =
        sqlx::query_as::<_, (i64, i64, Vec<u8>, String, String)>(
            "SELECT schema_version, fetched_at, payload, updated_at, typeof(payload) \
             FROM oauth_quota_snapshots WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .fetch_one(&mut connection)
        .await
        .expect("v11 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v11 payload JSON");

    assert_eq!(version, 11);
    assert_eq!(fetched_at, 1_787_137_660);
    assert_eq!(updated_at, "2026-08-20 08:30:00");
    assert_eq!(storage_class, "blob");
    assert_eq!(payload["usage"], representative_usage());
    assert_eq!(payload["estimator_state"], serde_json::Value::Null);
    assert_eq!(payload.as_object().map(serde_json::Map::len), Some(2));
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=40).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());

    let schema = table_schema_on_connection(&mut connection, "oauth_quota_snapshots").await;
    assert!(schema.contains("schema_version = 11"));
    assert!(
        sqlx::query(
            "UPDATE oauth_quota_snapshots SET schema_version = 10 WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .execute(&mut connection)
        .await
        .is_err()
    );
}

async fn insert_mixed_v10_snapshot(connection: &mut SqliteConnection) {
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
    let payload = json!({
        "usage": representative_usage(),
        "estimator_state": {
            "credential_fingerprint": "sha256:stable-principal",
            "subscription_tier": "plus",
            "windows": [{
                "key": {
                    "id": "primary",
                    "kind": "time",
                    "limit_window_seconds": 604_800
                },
                "cycle_started_at_ms": 1_786_827_681_000_u64,
                "cycle_reset_at": 1_787_432_481_i64,
                "local_cost_nanos": 3_352_000_000_000_u64,
                "capacity_eligible": true
            }]
        }
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 10, ?, CAST(? AS BLOB), ?)",
    )
    .bind(ACCOUNT_ID)
    .bind(1_787_137_660_i64)
    .bind(serde_json::to_vec(&payload).expect("v10 snapshot JSON"))
    .bind("2026-08-20 08:30:00")
    .execute(connection)
    .await
    .expect("representative v10 quota snapshot");
}

fn representative_usage() -> serde_json::Value {
    json!({
        "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "windows": [{
                "id": "primary",
                "kind": "time",
                "used_percent": 100.0,
                "limit_window_seconds": 604_800,
                "reset_after_seconds": 294_821,
                "reset_at": 1_787_432_481_i64
            }]
        },
        "credits": {
            "balance": "1320.53",
            "has_credits": true,
            "unlimited": false
        },
        "access": {
            "spend_control_reached": false,
            "reached_type": null
        },
        "reset_credits": {
            "available_count": 0,
            "credits": []
        },
        "billing": null,
        "token_balance": null,
        "subscription_tier": "plus",
        "account_status": null
    })
}
