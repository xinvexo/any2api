use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn response_body_bytes_migration_backfills_from_the_response_summary() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 13).await;
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, method, path, uri, http_version, \
          duration_ms, response_bytes, outcome, exchange_captured, request_headers, \
          request_body, response_headers, response_body) \
         VALUES ('legacy', 100, 1, 'POST', '/v1/responses', '/v1/responses', 'HTTP/1.1', \
                 1, 42, 'completed', 1, X'0102', X'0304', X'0506', X'0708')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy access log");

    migrate_through(&mut connection, 14).await;

    let backfilled: i64 = sqlx::query_scalar(
        "SELECT response_body_bytes FROM http_access_logs WHERE request_id = 'legacy'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("backfilled response body bytes");
    assert_eq!(backfilled, 42);
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=14).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
