use sqlx::{Connection, SqliteConnection};

use crate::{
    gateway_api_key::GATEWAY_API_KEY_USAGE_SUMMARY_SQL,
    request_log::UPSTREAM_CREDENTIAL_USAGE_SUMMARY_SQL,
};

use super::{DIRECT_PROXY_ID, foreign_key_violations, migrate_through, migration_versions};

const GATEWAY_ID: &str = "10000000-0000-4000-8000-000000000001";
const ENDPOINT_ID: &str = "20000000-0000-4000-8000-000000000001";
const CREDENTIAL_ID: &str = "30000000-0000-4000-8000-000000000001";
const OAUTH_ACCOUNT_ID: &str = "40000000-0000-4000-8000-000000000001";

#[tokio::test]
async fn request_usage_index_migration_preserves_rows_and_covers_aggregate_queries() {
    let mut connection = SqliteConnection::connect(":memory:")
        .await
        .expect("SQLite connection");
    migrate_through(&mut connection, 9).await;
    insert_representative_rows(&mut connection).await;

    assert_eq!(
        index_columns(&mut connection, "request_logs_gateway_key_started_idx").await,
        ["gateway_api_key_id", "started_at_ms", "request_id"]
    );
    assert_eq!(
        index_columns(
            &mut connection,
            "request_logs_provider_credential_started_idx"
        )
        .await,
        ["credential_id", "started_at_ms", "request_id"]
    );
    assert_eq!(
        index_columns(&mut connection, "request_logs_oauth_account_idx").await,
        ["oauth_account_id", "started_at_ms"]
    );

    migrate_through(&mut connection, 10).await;

    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=10).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    assert_index_columns(&mut connection).await;
    assert_aggregate_results(&mut connection).await;
    assert_covering_query_plans(&mut connection).await;
}

async fn insert_representative_rows(connection: &mut SqliteConnection) {
    let gateway_token = format!("a2k_v1_{}", "a".repeat(43));
    sqlx::query(
        "INSERT INTO gateway_api_keys \
         (id, name, name_key, token, token_prefix, token_hash, hash_version, token_version, \
          config_version, enabled) VALUES (?, 'Gateway', 'gateway', ?, 'a2k_v1_', \
          zeroblob(32), 2, 1, 1, 1)",
    )
    .bind(GATEWAY_ID)
    .bind(gateway_token)
    .execute(&mut *connection)
    .await
    .expect("gateway API key");
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, enabled, config_version) \
         VALUES (?, 'Codex', 'codex', 'codex', 'https://api.example.com', \
                 'openai_responses', 1, 1)",
    )
    .bind(ENDPOINT_ID)
    .execute(&mut *connection)
    .await
    .expect("provider endpoint");
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_version, \
          credential_generation, config_version, api_key, fingerprint_version, \
          secret_fingerprint, proxy_profile_id, enabled) \
         VALUES (?, ?, 'Key', 'key', 'api_key', 1, 1, 1, X'736B', 2, zeroblob(32), ?, 1)",
    )
    .bind(CREDENTIAL_ID)
    .bind(ENDPOINT_ID)
    .bind(DIRECT_PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("provider credential");
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, account_generation, \
          config_version, enabled) \
         VALUES (?, 'codex', 'OAuth', 'oauth', X'7B7D', 1, 1, 1, 1)",
    )
    .bind(OAUTH_ACCOUNT_ID)
    .execute(&mut *connection)
    .await
    .expect("OAuth account");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, gateway_api_key_id, ingress_protocol, \
          operation, credential_id, oauth_account_id, status_code, attempt_count, latency_ms, \
          is_stream, client_ip) VALUES \
         ('50000000-0000-4000-8000-000000000001', 1000, 1, ?, 'openai_responses', \
          'responses', ?, NULL, 200, 1, 1, 0, '127.0.0.1'), \
         ('50000000-0000-4000-8000-000000000002', 2000, 1, ?, 'openai_responses', \
          'responses', ?, NULL, 500, 1, 1, 0, '127.0.0.1'), \
         ('50000000-0000-4000-8000-000000000003', 3000, 1, ?, 'openai_responses', \
          'responses', NULL, ?, 201, 1, 1, 0, '127.0.0.1'), \
         ('50000000-0000-4000-8000-000000000004', 4000, 1, ?, 'openai_responses', \
          'responses', NULL, ?, 503, 1, 1, 0, '127.0.0.1'), \
         ('50000000-0000-4000-8000-000000000005', 5000, 1, NULL, 'openai_responses', \
          'responses', NULL, NULL, 200, 0, 1, 0, '127.0.0.1')",
    )
    .bind(GATEWAY_ID)
    .bind(CREDENTIAL_ID)
    .bind(GATEWAY_ID)
    .bind(CREDENTIAL_ID)
    .bind(GATEWAY_ID)
    .bind(OAUTH_ACCOUNT_ID)
    .bind(GATEWAY_ID)
    .bind(OAUTH_ACCOUNT_ID)
    .execute(connection)
    .await
    .expect("representative request logs");
}

async fn assert_index_columns(connection: &mut SqliteConnection) {
    assert_eq!(
        index_columns(connection, "request_logs_gateway_key_started_idx").await,
        [
            "gateway_api_key_id",
            "started_at_ms",
            "request_id",
            "status_code"
        ]
    );
    assert_eq!(
        index_columns(connection, "request_logs_provider_credential_started_idx").await,
        [
            "credential_id",
            "started_at_ms",
            "request_id",
            "status_code"
        ]
    );
    assert_eq!(
        index_columns(connection, "request_logs_oauth_account_idx").await,
        ["oauth_account_id", "started_at_ms", "status_code"]
    );
}

async fn assert_aggregate_results(connection: &mut SqliteConnection) {
    let upstream =
        sqlx::query_as::<_, (String, String, i64, i64)>(UPSTREAM_CREDENTIAL_USAGE_SUMMARY_SQL)
            .fetch_all(&mut *connection)
            .await
            .expect("upstream aggregate");
    assert_eq!(
        upstream,
        [
            ("provider_credential".into(), CREDENTIAL_ID.into(), 2, 1),
            ("oauth_account".into(), OAUTH_ACCOUNT_ID.into(), 2, 1),
        ]
    );

    let gateway = sqlx::query_as::<_, (String, i64, i64)>(GATEWAY_API_KEY_USAGE_SUMMARY_SQL)
        .fetch_all(connection)
        .await
        .expect("gateway aggregate");
    assert_eq!(gateway, [(GATEWAY_ID.into(), 4, 2)]);
}

async fn assert_covering_query_plans(connection: &mut SqliteConnection) {
    let upstream = query_plan(connection, UPSTREAM_CREDENTIAL_USAGE_SUMMARY_SQL).await;
    assert!(
        upstream.contains("COVERING INDEX request_logs_provider_credential_started_idx"),
        "{upstream}"
    );
    assert!(
        upstream.contains("COVERING INDEX request_logs_oauth_account_idx"),
        "{upstream}"
    );
    assert_request_log_access_is_covering(&upstream);

    let gateway = query_plan(connection, GATEWAY_API_KEY_USAGE_SUMMARY_SQL).await;
    assert!(
        gateway.contains("COVERING INDEX request_logs_gateway_key_started_idx"),
        "{gateway}"
    );
    assert_request_log_access_is_covering(&gateway);
}

fn assert_request_log_access_is_covering(plan: &str) {
    assert!(!plan.contains("USE TEMP B-TREE"), "{plan}");
    for line in plan.lines().filter(|line| line.contains("request_logs")) {
        assert!(line.contains("COVERING INDEX"), "{plan}");
    }
}

async fn index_columns(connection: &mut SqliteConnection, index: &str) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM pragma_index_info(?) ORDER BY seqno")
        .bind(index)
        .fetch_all(connection)
        .await
        .expect("index columns")
}

async fn query_plan(connection: &mut SqliteConnection, statement: &str) -> String {
    sqlx::query_as::<_, (i64, i64, i64, String)>(&format!("EXPLAIN QUERY PLAN {statement}"))
        .fetch_all(connection)
        .await
        .expect("query plan")
        .into_iter()
        .map(|(_, _, _, detail)| detail)
        .collect::<Vec<_>>()
        .join("\n")
}
