use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::tempdir;

use super::{
    foreign_key_violations, migrate_through, migration_versions, table_schema_on_connection,
};

#[tokio::test]
async fn request_log_cache_write_token_migration_preserves_existing_rows() {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("request-log-upgrade.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("SQLite pool");
    let mut connection = pool.acquire().await.expect("migration connection");
    migrate_through(&mut connection, 1).await;
    assert!(
        table_schema_on_connection(&mut connection, "request_logs")
            .await
            .contains("cache_write_tokens")
    );

    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, config_revision, \
         ingress_protocol, operation, status_code, attempt_count, latency_ms, input_tokens, \
         output_tokens, cache_read_tokens, cache_write_tokens, is_stream, client_ip) \
         VALUES ('migration-log', 1000, 1, 'openai_responses', 'responses', 200, 1, 9, \
         11, 7, 3, 5, 0, '127.0.0.1')",
    )
    .execute(&mut *connection)
    .await
    .expect("legacy request log");

    migrate_through(&mut connection, 2).await;

    let usage = sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT input_tokens, output_tokens, cache_read_tokens FROM request_logs \
         WHERE request_id = 'migration-log'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("migrated request log");
    assert_eq!(usage, (11, 7, 3));
    assert!(
        !table_schema_on_connection(&mut connection, "request_logs")
            .await
            .contains("cache_write_tokens")
    );
    assert_eq!(migration_versions(&mut connection).await, vec![1, 2]);
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
