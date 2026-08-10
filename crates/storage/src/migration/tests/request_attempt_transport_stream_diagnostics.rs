use sqlx::{Connection, SqliteConnection};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn transport_stream_diagnostics_preserve_old_attempts_and_require_complete_transport() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 18).await;
    sqlx::query(
        "INSERT INTO request_logs (request_id, started_at_ms, config_revision, ingress_protocol, \
         operation, status_code, attempt_count, latency_ms, is_stream, client_ip) \
         VALUES ('legacy', 100, 1, 'openai_responses', 'responses', 200, 1, 9, 1, '127.0.0.1')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, started_at_ms, duration_ms, status_code, outcome) \
         VALUES ('legacy', 1, 100, 9, 200, 'success')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy request attempt");

    migrate_through(&mut connection, 19).await;

    let legacy = sqlx::query_as::<_, (Option<String>, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT transport_wire_profile_id, first_upstream_frame_ms, stream_commit_ms, \
         first_downstream_byte_ms FROM request_attempts \
         WHERE request_id = 'legacy' AND attempt_no = 1",
    )
    .fetch_one(&mut connection)
    .await
    .expect("migrated request attempt");
    assert_eq!(legacy, (None, None, None, None));

    sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, started_at_ms, duration_ms, \
         status_code, outcome, transport_wire_profile_id, transport_wire_profile_version, \
         transport_timeout_policy_version, transport_resolver_mode, transport_proxy_kind, \
         transport_connect_timeout_ms, transport_read_timeout_ms, \
         transport_pool_idle_timeout_ms, transport_routing_generation, \
         transport_authentication_version, transport_traffic_class, first_upstream_frame_ms, \
         stream_commit_ms, first_downstream_byte_ms) VALUES \
         ('legacy', 2, 110, 8, 200, 'success', 'generic-rustls-hyper-v2', 2, 1, 'system', \
          'direct', 10000, 300000, 50000, 4, 7, 'data_plane', 2, 3, 5)",
    )
    .execute(&mut connection)
    .await
    .expect("diagnostic request attempt");
    let partial = sqlx::query(
        "INSERT INTO request_attempts (request_id, attempt_no, started_at_ms, duration_ms, \
         outcome, transport_wire_profile_id, transport_traffic_class) \
         VALUES ('legacy', 3, 120, 1, 'cancelled', 'generic-rustls-hyper-v2', 'data_plane')",
    )
    .execute(&mut connection)
    .await;
    assert!(partial.is_err());
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=19).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}
