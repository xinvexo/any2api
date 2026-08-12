use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

const ACCOUNT_ID: &str = "11111111-1111-4111-8111-111111111111";
const REQUEST_ID: &str = "22222222-2222-4222-8222-222222222222";
const OTHER_REQUEST_ID: &str = "33333333-3333-4333-8333-333333333333";
const PROCESS_ID: &str = "44444444-4444-4444-8444-444444444444";

#[tokio::test]
async fn quota_snapshot_v6_migration_preserves_usage_and_adds_monotonic_positions() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 25).await;
    insert_representative_v5_state(&mut connection).await;

    migrate_through(&mut connection, 26).await;

    assert_snapshot_migrated(&mut connection).await;
    assert_monotonic_position_schema(&mut connection).await;
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=26).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

async fn insert_representative_v5_state(connection: &mut SqliteConnection) {
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
        "estimator_state": {"legacy": "must be cleared"}
    });
    sqlx::query(
        "INSERT INTO oauth_quota_snapshots \
         (oauth_account_id, schema_version, fetched_at, payload, updated_at) \
         VALUES (?, 5, 1234, CAST(? AS BLOB), '2026-08-12 00:00:00')",
    )
    .bind(ACCOUNT_ID)
    .bind(serde_json::to_vec(&payload).expect("v5 payload"))
    .execute(&mut *connection)
    .await
    .expect("v5 quota snapshot");
    for request_id in [REQUEST_ID, OTHER_REQUEST_ID] {
        sqlx::query(
            "INSERT INTO request_logs \
             (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
              public_model, oauth_account_id, status_code, attempt_count, latency_ms, \
              is_stream, client_ip) \
             VALUES (?, 1200001, 1, 'openai_responses', 'responses', 'gpt-5.5', ?, \
                     200, 1, 10, 0, '127.0.0.1')",
        )
        .bind(request_id)
        .bind(ACCOUNT_ID)
        .execute(&mut *connection)
        .await
        .expect("legacy request log");
    }
}

async fn assert_snapshot_migrated(connection: &mut SqliteConnection) {
    let (version, payload, updated_at) = sqlx::query_as::<_, (i64, Vec<u8>, String)>(
        "SELECT schema_version, payload, updated_at FROM oauth_quota_snapshots \
         WHERE oauth_account_id = ?",
    )
    .bind(ACCOUNT_ID)
    .fetch_one(&mut *connection)
    .await
    .expect("v6 quota snapshot");
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("v6 payload JSON");
    assert_eq!(version, 6);
    assert_eq!(
        payload["usage"]["rate_limit"]["windows"][0]["used_percent"],
        42.0
    );
    assert!(payload["estimator_state"].is_null());
    assert_eq!(updated_at, "2026-08-12 00:00:00");
}

async fn assert_monotonic_position_schema(connection: &mut SqliteConnection) {
    let legacy = sqlx::query_as::<_, (Option<String>, Option<i64>)>(
        "SELECT telemetry_process_id, telemetry_sequence FROM request_logs \
         WHERE request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(&mut *connection)
    .await
    .expect("legacy position");
    assert_eq!(legacy, (None, None));
    sqlx::query(
        "UPDATE request_logs SET telemetry_process_id = ?, telemetry_sequence = 1 \
         WHERE request_id = ?",
    )
    .bind(PROCESS_ID)
    .bind(REQUEST_ID)
    .execute(&mut *connection)
    .await
    .expect("valid telemetry position");
    assert!(
        sqlx::query(
            "UPDATE request_logs SET telemetry_process_id = ?, telemetry_sequence = 1 \
             WHERE request_id = ?",
        )
        .bind(PROCESS_ID)
        .bind(OTHER_REQUEST_ID)
        .execute(&mut *connection)
        .await
        .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE request_logs SET telemetry_process_id = ?, telemetry_sequence = NULL \
             WHERE request_id = ?",
        )
        .bind(PROCESS_ID)
        .bind(OTHER_REQUEST_ID)
        .execute(&mut *connection)
        .await
        .is_err()
    );
    let sequence_index: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE type = 'index' \
         AND name = 'request_logs_oauth_quota_sequence_idx'",
    )
    .fetch_one(connection)
    .await
    .expect("OAuth quota sequence index");
    assert!(sequence_index.contains("telemetry_sequence"));
}
