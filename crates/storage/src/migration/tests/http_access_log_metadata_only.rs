use sqlx::{Connection, SqliteConnection};

use super::{migrate_through, migrator_through, table_schema_on_connection};

#[tokio::test]
async fn migration_rebuilds_metadata_only_logs_and_removes_raw_storage() {
    let mut connection = SqliteConnection::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 42).await;

    sqlx::query(
        "INSERT INTO http_access_logs (request_id, started_at_ms, config_revision, client_ip, \
         method, path, uri, http_version, status_code, duration_ms, response_bytes, outcome, \
         exchange_captured, request_headers, request_body, request_body_bytes, \
         request_body_complete, request_body_truncated, response_headers, response_body, \
         response_body_bytes, response_body_complete, response_body_truncated, exchange_bytes) \
         VALUES ('raw', 1, 1, NULL, 'POST', '/oauth/callback', \
         '/oauth/callback?code=secret', 'HTTP/1.1', 200, 1, 2, 'completed', 1, \
         X'5B7B226E616D65223A22617574686F72697A6174696F6E222C2276616C7565223A5B3131355D7D5D', \
         X'736563726574', 6, 1, 0, X'5B5D', X'6F6B', 2, 1, 0, 53)",
    )
    .execute(&mut connection)
    .await
    .expect("legacy raw access log");
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) \
         VALUES ('logs.http_access.max_exchange_bytes', '1048576')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy setting override");

    migrator_through(43)
        .run_direct(None, &mut connection, false)
        .await
        .expect("metadata-only migration");

    let row = sqlx::query_as::<_, (String, i64)>(
        "SELECT path, gateway_auth_rejected FROM http_access_logs WHERE request_id = 'raw'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("scrubbed access log");
    assert_eq!(row, ("/oauth/callback".to_owned(), 0));

    let schema = table_schema_on_connection(&mut connection, "http_access_logs").await;
    for removed in [
        "uri TEXT",
        "exchange_captured",
        "request_headers",
        "request_body",
        "response_headers",
        "response_body",
        "exchange_bytes",
    ] {
        assert!(
            !schema.contains(removed),
            "legacy column remains: {removed}"
        );
    }

    let stats = sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT request_log_rows, http_access_log_rows, gateway_auth_rejected_rows \
         FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_one(&mut connection)
    .await
    .expect("scrubbed capacity stats");
    assert_eq!(stats, (0, 1, 0));

    sqlx::query(
        "INSERT INTO http_access_logs (request_id, started_at_ms, config_revision, client_ip, \
         method, path, http_version, status_code, duration_ms, response_bytes, outcome, \
         gateway_auth_rejected) VALUES ('rejected', 2, 1, NULL, 'GET', '/v1/models', \
         'HTTP/1.1', 401, 1, 0, 'completed', 1)",
    )
    .execute(&mut connection)
    .await
    .expect("metadata-only access log");
    let stats = sqlx::query_as::<_, (i64, i64)>(
        "SELECT http_access_log_rows, gateway_auth_rejected_rows \
         FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_one(&mut connection)
    .await
    .expect("updated capacity stats");
    assert_eq!(stats, (2, 1));

    let override_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM setting_overrides \
         WHERE key = 'logs.http_access.max_exchange_bytes'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("removed override count");
    assert_eq!(override_count, 0);
}
