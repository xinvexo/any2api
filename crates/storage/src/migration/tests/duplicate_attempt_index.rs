use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn duplicate_attempt_index_migration_preserves_data_and_primary_key_lookup() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 7).await;
    insert_representative_rows(&mut connection).await;

    assert_eq!(
        index_columns(&mut connection, "request_attempts_request_idx").await,
        ["request_id", "attempt_no"]
    );
    assert_eq!(
        index_columns(&mut connection, "sqlite_autoindex_request_attempts_1").await,
        ["request_id", "attempt_no"]
    );

    migrate_through(&mut connection, 8).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        vec![1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    let attempts: Vec<i64> = sqlx::query_scalar(
        "SELECT attempt_no FROM request_attempts WHERE request_id = ? ORDER BY attempt_no",
    )
    .bind("50000000-0000-4000-8000-000000000001")
    .fetch_all(&mut connection)
    .await
    .expect("preserved attempts");
    assert_eq!(attempts, [1, 2]);

    let indexes: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, origin FROM pragma_index_list('request_attempts') \
         WHERE name IN ('request_attempts_request_idx', \
                        'sqlite_autoindex_request_attempts_1') ORDER BY name",
    )
    .fetch_all(&mut connection)
    .await
    .expect("attempt indexes");
    assert_eq!(
        indexes,
        [("sqlite_autoindex_request_attempts_1".into(), "pk".into())]
    );

    let plan = query_plan(
        &mut connection,
        "EXPLAIN QUERY PLAN SELECT attempt_no FROM request_attempts \
         WHERE request_id = '50000000-0000-4000-8000-000000000001' ORDER BY attempt_no",
    )
    .await;
    assert!(
        plan.contains("COVERING INDEX sqlite_autoindex_request_attempts_1"),
        "{plan}"
    );

    let duplicate = sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, started_at_ms, duration_ms, outcome) \
         VALUES ('50000000-0000-4000-8000-000000000001', 1, 3, 1, 'success')",
    )
    .execute(&mut connection)
    .await;
    assert!(
        duplicate.is_err(),
        "the composite primary key must remain unique"
    );
}

async fn insert_representative_rows(connection: &mut SqliteConnection) {
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, status_code, \
          attempt_count, latency_ms, is_stream, client_ip) \
         VALUES ('50000000-0000-4000-8000-000000000001', 1, 1, 'openai_responses', \
                 'responses', 200, 2, 2, 0, '127.0.0.1')",
    )
    .execute(&mut *connection)
    .await
    .expect("request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, started_at_ms, duration_ms, outcome) VALUES \
         ('50000000-0000-4000-8000-000000000001', 2, 2, 1, 'success'), \
         ('50000000-0000-4000-8000-000000000001', 1, 1, 1, 'success')",
    )
    .execute(connection)
    .await
    .expect("request attempts");
}

async fn index_columns(connection: &mut SqliteConnection, index: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_index_info(?) ORDER BY seqno")
        .bind(index)
        .fetch_all(connection)
        .await
        .expect("index columns")
}

async fn query_plan(connection: &mut SqliteConnection, statement: &'static str) -> String {
    sqlx::query_as::<_, (i64, i64, i64, String)>(statement)
        .fetch_all(connection)
        .await
        .expect("query plan")
        .into_iter()
        .map(|(_, _, _, detail)| detail)
        .collect::<Vec<_>>()
        .join("\n")
}
