use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const ACCOUNT_ID: &str = "33333333-3333-4333-8333-333333333333";

#[tokio::test]
async fn quota_snapshot_v9_migration_accumulates_every_v8_sample() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 28).await;
    insert_representative_v8_state(&mut connection).await;

    migrate_through(&mut connection, 29).await;

    assert_snapshot_migrated(&mut connection).await;
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=29).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

async fn insert_representative_v8_state(connection: &mut SqliteConnection) {
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
    let anchor = serde_json::json!({
        "used_percent": 42.0,
        "reset_at": 1900000100,
        "telemetry": {
            "position": {
                "process_id": "44444444-4444-4444-8444-444444444444",
                "sequence": 11
            },
            "observed_at_ms": 1900000000000_u64,
            "checkpoint": {
                "process_id": "44444444-4444-4444-8444-444444444444",
                "account_queue_dropped_request_logs": 0,
                "account_storage_failed_request_logs": 0,
                "unattributed_lost_request_logs": 0,
                "pruned_through_sequence": 0
            }
        }
    });
    let payload = serde_json::json!({
        "usage": {
            "rate_limit": {
                "allowed": true,
                "limit_reached": false,
                "windows": [{
                    "id": "primary",
                    "kind": "time",
                    "used_percent": 42.0,
                    "limit_window_seconds": 18000,
                    "reset_after_seconds": 100,
                    "reset_at": 1900000100
                }]
            },
            "credits": null,
            "access": null,
            "reset_credits": null,
            "billing": null,
            "token_balance": null,
            "subscription_tier": "pro",
            "account_status": null
        },
        "estimator_state": {
            "credential_fingerprint": "identity-a",
            "subscription_tier": "pro",
            "next_epoch": 4,
            "windows": [{
                "key": {
                    "id": "primary",
                    "kind": "time",
                    "limit_window_seconds": 18000
                },
                "epoch": 3,
                "epoch_started_at_ms": 1900000000000_u64,
                "last_observation": anchor,
                "sample_anchor": anchor,
                "samples": [{
                    "capacity_credits": 1500.0,
                    "delta_used_percent": 4.0,
                    "local_cost_credits": 60.0,
                    "observed_at_ms": 1899990000000_u64,
                    "epoch": 2,
                    "rate_cards": ["old-rate-a"]
                }, {
                    "capacity_credits": 1600.0,
                    "delta_used_percent": 6.0,
                    "local_cost_credits": 96.0,
                    "observed_at_ms": 1900000000000_u64,
                    "epoch": 3,
                    "rate_cards": ["old-rate-b"]
                }],
                "latest_interval": {
                    "status": "reset_boundary",
                    "started_at": null,
                    "ended_at": 1900000000,
                    "delta_used_percent": null,
                    "local_cost_credits": null,
                    "unpriced_request_count": 0,
                    "queue_dropped_request_logs": 0,
                    "storage_failed_request_logs": 0,
                    "interval_pruned": false
                }
            }]
        }
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 8, 1234, CAST(? AS BLOB), '2026-08-13 00:00:00')",
    )
    .bind(ACCOUNT_ID)
    .bind(serde_json::to_vec(&payload).expect("v8 payload"))
    .execute(&mut *connection)
    .await
    .expect("v8 quota snapshot");
}

async fn assert_snapshot_migrated(connection: &mut SqliteConnection) {
    let (version, fetched_at, payload, updated_at) =
        sqlx::query_as::<_, (i64, i64, Vec<u8>, String)>(
            "SELECT schema_version, fetched_at, payload, updated_at FROM oauth_quota_snapshots \
             WHERE oauth_account_id = ?",
        )
        .bind(ACCOUNT_ID)
        .fetch_one(&mut *connection)
        .await
        .expect("v9 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v9 payload JSON");
    let window = &payload["estimator_state"]["windows"][0];
    assert_eq!(version, 9);
    assert_eq!(fetched_at, 1234);
    assert_eq!(window["anchor"]["used_percent"], 42.0);
    assert_eq!(window["anchor"]["reset_at"], 1_900_000_100_i64);
    assert_eq!(window["anchor"]["position"]["sequence"], 11);
    assert_eq!(window["total_delta_used_percent"], 10.0);
    assert_eq!(window["total_local_cost_credits"], 156.0);
    assert_eq!(window["completed_interval_count"], 2);
    assert!(window.get("samples").is_none());
    assert!(window.get("epoch").is_none());
    assert!(window.get("latest_interval").is_none());
    assert_eq!(updated_at, "2026-08-13 00:00:00");
    assert!(
        sqlx::query(
            "INSERT INTO oauth_quota_snapshots \
             (oauth_account_id, schema_version, fetched_at, payload) \
             VALUES ('55555555-5555-4555-8555-555555555555', 8, 1, CAST('{}' AS BLOB))",
        )
        .execute(connection)
        .await
        .is_err()
    );
}
