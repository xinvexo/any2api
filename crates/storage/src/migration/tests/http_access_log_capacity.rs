use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn capacity_migration_backfills_exchange_bytes_and_preserves_history() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 6).await;
    insert_access_log(&mut connection, "captured", 100, true).await;
    insert_access_log(&mut connection, "legacy", 200, false).await;

    migrate_through(&mut connection, 7).await;

    let rows = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT request_id, exchange_captured, exchange_bytes FROM http_access_logs \
         ORDER BY started_at_ms",
    )
    .fetch_all(&mut connection)
    .await
    .expect("migrated access logs");
    assert_eq!(
        rows,
        vec![("captured".into(), 1, 24), ("legacy".into(), 0, 0)]
    );
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=7).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_index_info('http_access_logs_retention_idx') \
                            ORDER BY seqno",
    )
    .fetch_all(&mut connection)
    .await
    .expect("retention index columns");
    assert_eq!(columns, ["started_at_ms", "request_id", "exchange_bytes"]);
    let plan = sqlx::query_as::<_, (i64, i64, i64, String)>(
        "EXPLAIN QUERY PLAN SELECT COUNT(*), COALESCE(SUM(exchange_bytes), 0) \
         FROM http_access_logs INDEXED BY http_access_logs_retention_idx",
    )
    .fetch_all(&mut connection)
    .await
    .expect("capacity query plan")
    .into_iter()
    .map(|(_, _, _, detail)| detail)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        plan.contains("USING COVERING INDEX http_access_logs_retention_idx"),
        "{plan}"
    );
}

async fn insert_access_log(
    connection: &mut SqliteConnection,
    request_id: &str,
    started_at_ms: i64,
    captured: bool,
) {
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, method, path, uri, http_version, \
          duration_ms, response_bytes, outcome, exchange_captured, request_headers, \
          request_body, response_headers, response_body) \
         VALUES (?, ?, 1, 'POST', '/v1/responses', '/v1/responses?raw=1', 'HTTP/1.1', \
                 1, 11, 'completed', ?, X'010203', X'0405060708', X'090A0B0C', \
                 X'0D0E0F101112131415161718')",
    )
    .bind(request_id)
    .bind(started_at_ms)
    .bind(i64::from(captured))
    .execute(connection)
    .await
    .expect("representative access log");
}
