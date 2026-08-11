use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const ACCOUNT_ID: &str = "11111111-1111-4111-8111-111111111111";
const REQUEST_ID: &str = "22222222-2222-4222-8222-222222222222";

#[tokio::test]
async fn quota_snapshot_v5_migration_keeps_usage_and_starts_epoch_learning_cleanly() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 24).await;
    insert_representative_v4_state(&mut connection).await;

    migrate_through(&mut connection, 25).await;

    assert_snapshot_migrated(&mut connection).await;
    assert_frozen_cost_schema(&mut connection).await;
    assert_legacy_boundary_removed(&mut connection).await;
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=25).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

async fn insert_representative_v4_state(connection: &mut SqliteConnection) {
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
            "subscription_tier": null,
            "account_status": null
        },
        "usd_estimates": [{
            "window_id": "primary",
            "estimated_capacity_usd": 99.0
        }]
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 4, 1234, CAST(? AS BLOB), '2026-08-11 00:00:00')",
    )
    .bind(ACCOUNT_ID)
    .bind(serde_json::to_vec(&payload).expect("v4 payload"))
    .execute(&mut *connection)
    .await
    .expect("v4 quota snapshot");
    sqlx::query(
        "INSERT INTO oauth_quota_estimation_boundaries (oauth_account_id, reset_at_ms) \
         VALUES (?, 1200000)",
    )
    .bind(ACCOUNT_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy reset boundary");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, public_model, \
          oauth_account_id, status_code, attempt_count, latency_ms, is_stream, client_ip) \
         VALUES (?, 1200001, 1, 'openai_responses', 'responses', 'gpt-5.5', ?, \
                 200, 1, 10, 0, '127.0.0.1')",
    )
    .bind(REQUEST_ID)
    .bind(ACCOUNT_ID)
    .execute(connection)
    .await
    .expect("legacy request log");
}

async fn assert_snapshot_migrated(connection: &mut SqliteConnection) {
    let (version, payload, updated_at) = sqlx::query_as::<_, (i64, Vec<u8>, String)>(
        "SELECT schema_version, payload, updated_at FROM oauth_quota_snapshots \
         WHERE oauth_account_id = ?",
    )
    .bind(ACCOUNT_ID)
    .fetch_one(&mut *connection)
    .await
    .expect("v5 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v5 payload JSON");
    assert_eq!(version, 5);
    assert_eq!(
        payload["usage"]["rate_limit"]["windows"][0]["used_percent"],
        42.0
    );
    assert!(payload["estimator_state"].is_null());
    assert!(payload.get("usd_estimates").is_none());
    assert_eq!(updated_at, "2026-08-11 00:00:00");
}

async fn assert_frozen_cost_schema(connection: &mut SqliteConnection) {
    let frozen_cost =
        sqlx::query_as::<_, (Option<String>, Option<i64>, Option<String>, Option<String>)>(
            "SELECT quota_cost_unit, quota_cost_nanos, quota_cost_rate_card, quota_service_tier \
             FROM request_logs WHERE request_id = ?",
        )
        .bind(REQUEST_ID)
        .fetch_one(&mut *connection)
        .await
        .expect("migrated request log");
    assert_eq!(frozen_cost, (None, None, None, None));
    assert!(
        sqlx::query(
            "UPDATE request_logs SET quota_cost_unit = 'codex_credits', quota_cost_nanos = 1, \
             quota_cost_rate_card = 'test', quota_service_tier = 'standard' \
             WHERE request_id = ?",
        )
        .bind(REQUEST_ID)
        .execute(&mut *connection)
        .await
        .is_ok()
    );
    assert!(
        sqlx::query("UPDATE request_logs SET quota_cost_nanos = NULL WHERE request_id = ?")
            .bind(REQUEST_ID)
            .execute(&mut *connection)
            .await
            .is_err()
    );
    let quota_completion_index: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE type = 'index' \
         AND name = 'request_logs_oauth_quota_completion_idx'",
    )
    .fetch_one(connection)
    .await
    .expect("quota completion index");
    assert!(quota_completion_index.contains("started_at_ms + latency_ms"));
}

async fn assert_legacy_boundary_removed(connection: &mut SqliteConnection) {
    let boundary_exists: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_schema WHERE type = 'table' \
         AND name = 'oauth_quota_estimation_boundaries'",
    )
    .fetch_optional(connection)
    .await
    .expect("removed boundary lookup");
    assert_eq!(boundary_exists, None);
}
