use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn capacity_stats_migration_aggregates_history_and_triggers_stay_in_sync() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 14).await;
    insert_access_log(&mut connection, "normal", 100, 30, false).await;
    insert_access_log(&mut connection, "rejected", 200, 7, true).await;
    insert_request_log(&mut connection, "seeded", 300).await;

    migrate_through(&mut connection, 15).await;

    assert_eq!(capacity_stats(&mut connection).await, (1, 2, 37, 1, 7));
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=15).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());

    insert_access_log(&mut connection, "post-migration", 400, 11, true).await;
    insert_request_log(&mut connection, "post-migration", 500).await;
    assert_eq!(capacity_stats(&mut connection).await, (2, 3, 48, 2, 18));

    sqlx::query("DELETE FROM http_access_logs WHERE request_id = 'rejected'")
        .execute(&mut connection)
        .await
        .expect("delete rejected access log");
    sqlx::query("DELETE FROM request_logs WHERE request_id = 'seeded'")
        .execute(&mut connection)
        .await
        .expect("delete seeded request log");
    assert_eq!(capacity_stats(&mut connection).await, (1, 2, 41, 1, 11));
}

async fn capacity_stats(connection: &mut SqliteConnection) -> (i64, i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT request_log_rows, http_access_log_rows, http_access_log_exchange_bytes, \
         gateway_auth_rejected_rows, gateway_auth_rejected_exchange_bytes \
         FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_one(connection)
    .await
    .expect("telemetry capacity stats")
}

async fn insert_access_log(
    connection: &mut SqliteConnection,
    request_id: &str,
    started_at_ms: i64,
    exchange_bytes: i64,
    gateway_auth_rejected: bool,
) {
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, method, path, uri, http_version, \
          duration_ms, response_bytes, outcome, exchange_captured, request_headers, \
          request_body, response_headers, response_body, exchange_bytes, gateway_auth_rejected) \
         VALUES (?, ?, 1, 'POST', '/v1/responses', '/v1/responses', 'HTTP/1.1', \
                 1, 11, 'completed', 1, X'0102', X'0304', X'0506', X'0708', ?, ?)",
    )
    .bind(request_id)
    .bind(started_at_ms)
    .bind(exchange_bytes)
    .bind(i64::from(gateway_auth_rejected))
    .execute(connection)
    .await
    .expect("representative access log");
}

async fn insert_request_log(
    connection: &mut SqliteConnection,
    request_id: &str,
    started_at_ms: i64,
) {
    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, config_revision, \
         ingress_protocol, operation, status_code, attempt_count, latency_ms, is_stream, \
         client_ip) \
         VALUES (?, ?, 1, 'openai_responses', 'responses', 200, 1, 9, 0, '127.0.0.1')",
    )
    .bind(request_id)
    .bind(started_at_ms)
    .execute(connection)
    .await
    .expect("representative request log");
}
