use sqlx::{Connection, SqliteConnection};

use super::{DIRECT_PROXY_ID, foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn query_index_migration_preserves_rows_and_index_definitions() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 5).await;
    insert_representative_rows(&mut connection).await;

    migrate_through(&mut connection, 6).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        vec![1, 2, 3, 4, 5, 6]
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    assert_representative_rows(&mut connection).await;
    assert_index_columns(&mut connection).await;
}

async fn insert_representative_rows(connection: &mut SqliteConnection) {
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, enabled, config_version) \
         VALUES ('10000000-0000-4000-8000-000000000001', 'Codex', 'codex', 'codex', \
                 'https://api.example.com', 'openai_responses', 1, 1)",
    )
    .execute(&mut *connection)
    .await
    .expect("endpoint");
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_version, \
          credential_generation, config_version, api_key, fingerprint_version, \
          secret_fingerprint, proxy_profile_id, enabled) \
         VALUES ('20000000-0000-4000-8000-000000000001', \
                 '10000000-0000-4000-8000-000000000001', 'Key', 'key', 'api_key', 1, 1, 1, \
                 X'736B', 2, zeroblob(32), ?, 1)",
    )
    .bind(DIRECT_PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("credential");
    sqlx::query(
        "INSERT INTO model_routes \
         (id, public_model, ingress_protocol, enabled, config_version) \
         VALUES ('30000000-0000-4000-8000-000000000001', 'gpt-test', \
                 'openai_responses', 1, 1)",
    )
    .execute(&mut *connection)
    .await
    .expect("route");
    sqlx::query(
        "INSERT INTO route_targets \
         (id, model_route_id, provider_endpoint_id, upstream_model, \
          upstream_protocol_dialect, fallback_tier, enabled) \
         VALUES ('40000000-0000-4000-8000-000000000001', \
                 '30000000-0000-4000-8000-000000000001', \
                 '10000000-0000-4000-8000-000000000001', 'gpt-test', \
                 'openai_responses', 0, 1)",
    )
    .execute(&mut *connection)
    .await
    .expect("route target");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          provider_endpoint_id, credential_id, proxy_profile_id, status_code, attempt_count, \
          latency_ms, is_stream, client_ip) \
         VALUES ('50000000-0000-4000-8000-000000000001', 1000, 1, 'openai_responses', \
                 'responses', '10000000-0000-4000-8000-000000000001', \
                 '20000000-0000-4000-8000-000000000001', ?, 200, 1, 1, 0, '127.0.0.1')",
    )
    .bind(DIRECT_PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, route_target_id, credential_id, proxy_profile_id, \
          started_at_ms, duration_ms, outcome) \
         VALUES ('50000000-0000-4000-8000-000000000001', 1, \
                 '40000000-0000-4000-8000-000000000001', \
                 '20000000-0000-4000-8000-000000000001', ?, 1000, 1, 'success')",
    )
    .bind(DIRECT_PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("request attempt");
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, client_ip, method, path, http_version, \
          status_code, duration_ms, response_bytes, outcome, uri, exchange_captured, \
          request_body, response_body) \
         VALUES ('60000000-0000-4000-8000-000000000001', 1000, 1, '127.0.0.1', 'POST', \
                 '/v1/responses', 'HTTP/1.1', 200, 1, 1048576, 'completed', '/v1/responses', \
                 1, zeroblob(1048576), zeroblob(1048576))",
    )
    .execute(connection)
    .await
    .expect("HTTP access log");
}

async fn assert_representative_rows(connection: &mut SqliteConnection) {
    let target: Option<String> = sqlx::query_scalar(
        "SELECT route_target_id FROM request_attempts \
         WHERE request_id = '50000000-0000-4000-8000-000000000001'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("attempt target");
    assert_eq!(
        target.as_deref(),
        Some("40000000-0000-4000-8000-000000000001")
    );
    let body_bytes: i64 = sqlx::query_scalar(
        "SELECT length(request_body) + length(response_body) FROM http_access_logs",
    )
    .fetch_one(connection)
    .await
    .expect("body bytes");
    assert_eq!(body_bytes, 2 * 1_048_576);
}

async fn assert_index_columns(connection: &mut SqliteConnection) {
    for (index, columns) in [
        (
            "request_attempts_route_target_idx",
            &["route_target_id"][..],
        ),
        ("request_attempts_credential_idx", &["credential_id"][..]),
        (
            "request_attempts_proxy_profile_idx",
            &["proxy_profile_id"][..],
        ),
        (
            "request_logs_provider_endpoint_idx",
            &["provider_endpoint_id"][..],
        ),
        ("request_logs_proxy_profile_idx", &["proxy_profile_id"][..]),
        (
            "http_access_logs_summary_filter_idx",
            &[
                "started_at_ms",
                "request_id",
                "path",
                "client_ip",
                "status_code",
                "outcome",
            ][..],
        ),
    ] {
        let actual: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_index_info(?) ORDER BY seqno")
                .bind(index)
                .fetch_all(&mut *connection)
                .await
                .expect("index columns");
        assert_eq!(actual, columns, "{index}");
    }
}
