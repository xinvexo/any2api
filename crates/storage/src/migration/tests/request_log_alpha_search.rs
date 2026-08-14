use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{foreign_key_violations, migrate_through, migration_versions};

const REQUEST_ID: &str = "50000000-0000-4000-8000-000000000032";
const PROCESS_ID: &str = "00000000-0000-4000-8000-000000000032";

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct RequestLogSnapshot {
    request_id: String,
    started_at_ms: i64,
    config_revision: i64,
    gateway_api_key_id: Option<String>,
    ingress_protocol: String,
    operation: String,
    public_model: Option<String>,
    provider_endpoint_id: Option<String>,
    credential_id: Option<String>,
    oauth_account_id: Option<String>,
    proxy_profile_id: Option<String>,
    status_code: i64,
    error_class: Option<String>,
    error_message: Option<String>,
    attempt_count: i64,
    latency_ms: i64,
    first_token_ms: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_tokens: Option<i64>,
    thinking_level: Option<String>,
    is_stream: i64,
    client_ip: String,
    created_at: String,
    quota_cost_unit: Option<String>,
    quota_cost_nanos: Option<i64>,
    quota_cost_rate_card: Option<String>,
    quota_service_tier: Option<String>,
    telemetry_process_id: Option<String>,
    telemetry_sequence: Option<i64>,
    cache_creation_tokens: Option<i64>,
}

#[tokio::test]
async fn alpha_search_operation_migration_extends_the_check_and_preserves_rows() {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection");
    migrate_through(&mut connection, 31).await;
    insert_legacy_log(&mut connection).await;
    let rejected = insert_operation_log(&mut connection, "req-alpha-early", "alpha_search")
        .await
        .expect_err("pre-migration schema must reject alpha_search");
    assert!(
        rejected.to_string().contains("CHECK constraint failed"),
        "unexpected rejection: {rejected}"
    );
    let snapshot_before = log_snapshot(&mut connection).await;
    assert_eq!(capacity_rows(&mut connection).await, 1);

    migrate_through(&mut connection, 32).await;

    assert_eq!(log_snapshot(&mut connection).await, snapshot_before);
    assert_eq!(snapshot_before.operation, "responses");
    assert_eq!(snapshot_before.created_at, "2026-03-04 05:06:07");
    assert_eq!(attempt_count(&mut connection).await, 1);
    assert_eq!(capacity_rows(&mut connection).await, 1);
    assert_eq!(schema_count(&mut connection, "index").await, 9);
    assert_eq!(schema_count(&mut connection, "trigger").await, 2);
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=32).collect::<Vec<_>>()
    );

    insert_operation_log(&mut connection, "req-alpha", "alpha_search")
        .await
        .expect("alpha_search log after migration");
    assert_eq!(capacity_rows(&mut connection).await, 2);
}

async fn insert_legacy_log(connection: &mut SqliteConnection) {
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          public_model, status_code, error_class, error_message, attempt_count, latency_ms, \
          first_token_ms, input_tokens, output_tokens, cache_read_tokens, thinking_level, \
          is_stream, client_ip, created_at, quota_cost_unit, quota_cost_nanos, \
          quota_cost_rate_card, quota_service_tier, telemetry_process_id, telemetry_sequence, \
          cache_creation_tokens) \
         VALUES (?, 1000, 5, 'openai_responses', 'responses', 'gpt-search-model', 502, \
                 'upstream', 'upstream failed', 2, 90, 15, 120, 45, 30, 'max', 1, \
                 '127.0.0.1', '2026-03-04 05:06:07', 'codex_credits', 42, 'card/v1', \
                 'standard', ?, 1, 7)",
    )
    .bind(REQUEST_ID)
    .bind(PROCESS_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, started_at_ms, duration_ms, outcome) \
         VALUES (?, 1, 1001, 25, 'upstream_error')",
    )
    .bind(REQUEST_ID)
    .execute(connection)
    .await
    .expect("legacy request attempt");
}

async fn insert_operation_log(
    connection: &mut SqliteConnection,
    request_id: &str,
    operation: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          status_code, attempt_count, latency_ms, is_stream, client_ip) \
         VALUES (?, 2000, 5, 'openai_responses', ?, 200, 1, 9, 0, '127.0.0.1')",
    )
    .bind(request_id)
    .bind(operation)
    .execute(connection)
    .await
    .map(drop)
}

async fn log_snapshot(connection: &mut SqliteConnection) -> RequestLogSnapshot {
    sqlx::query_as(
        "SELECT request_id, started_at_ms, config_revision, gateway_api_key_id, \
         ingress_protocol, operation, public_model, provider_endpoint_id, credential_id, \
         oauth_account_id, proxy_profile_id, status_code, error_class, error_message, \
         attempt_count, latency_ms, first_token_ms, input_tokens, output_tokens, \
         cache_read_tokens, thinking_level, is_stream, client_ip, created_at, \
         quota_cost_unit, quota_cost_nanos, quota_cost_rate_card, quota_service_tier, \
         telemetry_process_id, telemetry_sequence, cache_creation_tokens \
         FROM request_logs WHERE request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(connection)
    .await
    .expect("request log snapshot")
}

async fn attempt_count(connection: &mut SqliteConnection) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM request_attempts WHERE request_id = ?")
        .bind(REQUEST_ID)
        .fetch_one(connection)
        .await
        .expect("attempt count")
}

async fn capacity_rows(connection: &mut SqliteConnection) -> i64 {
    sqlx::query_scalar(
        "SELECT request_log_rows FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_one(connection)
    .await
    .expect("capacity stats")
}

async fn schema_count(connection: &mut SqliteConnection, kind: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_schema \
         WHERE type = ? AND tbl_name = 'request_logs' AND name LIKE 'request_logs_%'",
    )
    .bind(kind)
    .fetch_one(connection)
    .await
    .expect("schema object count")
}
