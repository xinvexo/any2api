use sqlx::{AssertSqlSafe, Connection, SqliteConnection};

use crate::http_access_log::{HIDE_ADMIN_OPERATIONS_PREDICATE, SYSTEM_LOG_RETENTION_PREDICATE};

use super::{DIRECT_PROXY_ID, foreign_key_violations, migrate_through, migration_versions};

#[tokio::test]
async fn query_index_migration_preserves_rows_and_covers_fk_deletes_and_log_filters() {
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
    assert_covering_query_plans(&mut connection).await;
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

async fn assert_covering_query_plans(connection: &mut SqliteConnection) {
    let count_statement = format!(
        "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM http_access_logs \
         INDEXED BY http_access_logs_summary_filter_idx \
         WHERE started_at_ms >= 0 AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
         AND ({HIDE_ADMIN_OPERATIONS_PREDICATE}) \
         AND (started_at_ms, request_id) <= (1000, 'ffffffff-ffff-ffff-ffff-ffffffffffff')"
    );
    let count_plan = query_plan(connection, &count_statement).await;
    assert!(count_plan.contains("USING COVERING INDEX http_access_logs_summary_filter_idx"));

    let page_statement = format!(
        "EXPLAIN QUERY PLAN SELECT request_id, started_at_ms, config_revision, client_ip, \
         method, path, uri, http_version, status_code, duration_ms, response_bytes, outcome, \
         exchange_captured FROM http_access_logs \
         INDEXED BY http_access_logs_summary_filter_idx WHERE started_at_ms >= 0 AND (\
         {SYSTEM_LOG_RETENTION_PREDICATE}) \
         AND (started_at_ms, request_id) <= (1000, 'ffffffff-ffff-ffff-ffff-ffffffffffff') \
         AND (started_at_ms, request_id) < (500, 'ffffffff-ffff-ffff-ffff-ffffffffffff') \
         ORDER BY started_at_ms DESC, request_id DESC LIMIT 21"
    );
    let page_plan = query_plan(connection, &page_statement).await;
    assert!(
        page_plan.contains("USING INDEX http_access_logs_summary_filter_idx"),
        "{page_plan}"
    );

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await
        .expect("foreign keys");
    let delete_plan = query_plan(
        connection,
        "EXPLAIN QUERY PLAN DELETE FROM route_targets \
         WHERE id = '40000000-0000-4000-8000-000000000001'",
    )
    .await;
    assert!(delete_plan.contains("request_attempts_route_target_idx"));

    let delete_plan = query_plan(
        connection,
        "EXPLAIN QUERY PLAN DELETE FROM provider_credentials \
         WHERE id = '20000000-0000-4000-8000-000000000001'",
    )
    .await;
    assert!(delete_plan.contains("request_attempts_credential_idx"));

    let delete_plan = query_plan(
        connection,
        "EXPLAIN QUERY PLAN DELETE FROM provider_endpoints \
         WHERE id = '10000000-0000-4000-8000-000000000001'",
    )
    .await;
    assert!(delete_plan.contains("request_logs_provider_endpoint_idx"));

    let delete_plan = query_plan(
        connection,
        "EXPLAIN QUERY PLAN DELETE FROM proxy_profiles \
         WHERE id = '00000000-0000-0000-0000-000000000000'",
    )
    .await;
    assert!(delete_plan.contains("request_attempts_proxy_profile_idx"));
    assert!(delete_plan.contains("request_logs_proxy_profile_idx"));
}

async fn query_plan(connection: &mut SqliteConnection, sql: &str) -> String {
    sqlx::query_as::<_, (i64, i64, i64, String)>(AssertSqlSafe(sql))
        .fetch_all(connection)
        .await
        .expect("query plan")
        .into_iter()
        .map(|(_, _, _, detail)| detail)
        .collect::<Vec<_>>()
        .join("\n")
}
