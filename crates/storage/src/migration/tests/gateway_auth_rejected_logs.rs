use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn gateway_auth_rejected_migration_preserves_history_and_adds_covering_index() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 10).await;
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, method, path, uri, http_version, \
          status_code, duration_ms, response_bytes, outcome, exchange_bytes) \
         VALUES ('legacy-auth-result', 100, 1, 'GET', '/v1/models', '/v1/models', \
                 'HTTP/1.1', 401, 1, 42, 'completed', 19)",
    )
    .execute(&mut connection)
    .await
    .expect("representative legacy access log");

    migrate_through(&mut connection, 11).await;

    let migrated = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT request_id, gateway_auth_rejected, exchange_bytes \
         FROM http_access_logs",
    )
    .fetch_one(&mut connection)
    .await
    .expect("migrated access log");
    assert_eq!(migrated, ("legacy-auth-result".into(), 0, 19));
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=11).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());

    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_index_info(\
            'http_access_logs_gateway_auth_rejected_retention_idx') ORDER BY seqno",
    )
    .fetch_all(&mut connection)
    .await
    .expect("gateway rejection retention index columns");
    assert_eq!(
        columns,
        [
            "gateway_auth_rejected",
            "started_at_ms",
            "request_id",
            "exchange_bytes",
        ]
    );
    let plan = sqlx::query_as::<_, (i64, i64, i64, String)>(
        "EXPLAIN QUERY PLAN SELECT request_id, exchange_bytes FROM http_access_logs \
         INDEXED BY http_access_logs_gateway_auth_rejected_retention_idx \
         WHERE gateway_auth_rejected = 1 \
         ORDER BY started_at_ms ASC, request_id ASC",
    )
    .fetch_all(&mut connection)
    .await
    .expect("gateway rejection capacity query plan")
    .into_iter()
    .map(|(_, _, _, detail)| detail)
    .collect::<Vec<_>>()
    .join("\n");
    assert!(
        plan.contains("USING COVERING INDEX http_access_logs_gateway_auth_rejected_retention_idx"),
        "{plan}"
    );
}
