use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn routing_diagnostics_migration_preserves_old_attempts_and_validates_new_values() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 16).await;
    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, config_revision, ingress_protocol, \
         operation, status_code, attempt_count, latency_ms, is_stream, client_ip) \
         VALUES ('legacy', 100, 1, 'openai_responses', 'responses', 401, 1, 9, 0, '127.0.0.1')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy request log");
    sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, started_at_ms, duration_ms, \
         retry_safety, status_code, outcome) VALUES \
         ('legacy', 1, 100, 9, 'rejected_before_execution', 401, 'upstream_error')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy request attempt");

    migrate_through(&mut connection, 17).await;

    let legacy = sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>)>(
        "SELECT routing_mode, failure_scope, retry_decision FROM request_attempts \
         WHERE request_id = 'legacy' AND attempt_no = 1",
    )
    .fetch_one(&mut connection)
    .await
    .expect("migrated request attempt");
    assert_eq!(legacy, (None, None, None));

    sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, started_at_ms, duration_ms, \
         retry_safety, status_code, outcome, routing_mode, failure_scope, retry_decision) VALUES \
         ('legacy', 2, 110, 4, 'rejected_before_execution', 401, 'upstream_error', \
          'balanced', 'authentication', 'reselect')",
    )
    .execute(&mut connection)
    .await
    .expect("diagnostic request attempt");
    let invalid = sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, started_at_ms, duration_ms, \
         outcome, routing_mode) VALUES ('legacy', 3, 120, 1, 'cancelled', 'random')",
    )
    .execute(&mut connection)
    .await;
    assert!(invalid.is_err());
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=17).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
