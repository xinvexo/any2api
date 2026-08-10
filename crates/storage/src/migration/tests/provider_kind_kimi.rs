use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{foreign_key_violations, migrate_through, migration_versions};

const ENDPOINT_ID: &str = "10000000-0000-4000-8000-000000000001";
const CREDENTIAL_ID: &str = "20000000-0000-4000-8000-000000000001";
const ROUTE_ID: &str = "30000000-0000-4000-8000-000000000001";
const TARGET_ID: &str = "40000000-0000-4000-8000-000000000001";
const REQUEST_ID: &str = "50000000-0000-4000-8000-000000000001";

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct EndpointSnapshot {
    id: String,
    name: String,
    name_key: String,
    provider_kind: String,
    base_url: String,
    protocol_dialect: String,
    upstream_protocol_dialect: Option<String>,
    enabled: i64,
    config_version: i64,
    created_at: String,
    updated_at: String,
}

#[tokio::test]
async fn kimi_provider_migration_preserves_related_rows_without_url_inference() {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection");
    migrate_through(&mut connection, 17).await;
    insert_legacy_graph(&mut connection).await;
    let endpoint_before = endpoint_snapshot(&mut connection).await;

    migrate_through(&mut connection, 18).await;

    assert_eq!(endpoint_snapshot(&mut connection).await, endpoint_before);
    assert_eq!(endpoint_before.provider_kind, "grok");
    assert_eq!(endpoint_before.base_url, "https://api.moonshot.cn/v1");
    assert_eq!(related_row_count(&mut connection).await, 5);
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=18).collect::<Vec<_>>()
    );

    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES ('60000000-0000-4000-8000-000000000001', 'Kimi', 'kimi', 'kimi', \
                 'https://api.moonshot.cn/v1', 'openai_responses', \
                 'openai_chat_completions', 1, 1)",
    )
    .execute(&mut connection)
    .await
    .expect("Kimi endpoint after migration");
}

async fn insert_legacy_graph(connection: &mut SqliteConnection) {
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version, created_at, updated_at) \
         VALUES (?, 'Moonshot mislabeled', 'moonshot mislabeled', 'grok', \
                 'https://api.moonshot.cn/v1', 'openai_responses', \
                 'openai_chat_completions', 1, 7, '2026-01-01 00:00:00', \
                 '2026-02-02 00:00:00')",
    )
    .bind(ENDPOINT_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy endpoint");
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_version, \
          credential_generation, config_version, api_key, fingerprint_version, \
          secret_fingerprint, secret_tail, proxy_profile_id, requests_per_minute, enabled) \
         VALUES (?, ?, 'Key', 'key', 'api_key', 2, 3, 4, CAST('secret' AS BLOB), 2, \
                 zeroblob(32), NULL, '00000000-0000-0000-0000-000000000000', 60, 1)",
    )
    .bind(CREDENTIAL_ID)
    .bind(ENDPOINT_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy credential");
    sqlx::query(
        "INSERT INTO provider_credential_models (credential_id, upstream_model) \
         VALUES (?, 'kimi-k3')",
    )
    .bind(CREDENTIAL_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy credential model");
    sqlx::query(
        "INSERT INTO model_routes \
         (id, public_model, ingress_protocol, fallback_on_rate_limit, enabled, config_version) \
         VALUES (?, 'kimi-k3', 'openai_responses', NULL, 1, 5)",
    )
    .bind(ROUTE_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy route");
    sqlx::query(
        "INSERT INTO route_targets \
         (id, model_route_id, provider_endpoint_id, upstream_model, \
          upstream_protocol_dialect, fallback_tier, enabled) \
         VALUES (?, ?, ?, 'kimi-k3', 'openai_chat_completions', 0, 1)",
    )
    .bind(TARGET_ID)
    .bind(ROUTE_ID)
    .bind(ENDPOINT_ID)
    .execute(&mut *connection)
    .await
    .expect("legacy target");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, public_model, \
          provider_endpoint_id, credential_id, status_code, attempt_count, latency_ms, is_stream, \
          client_ip) \
         VALUES (?, 1000, 5, 'openai_responses', 'responses', 'kimi-k3', ?, ?, 200, 1, 9, 0, \
                 '127.0.0.1')",
    )
    .bind(REQUEST_ID)
    .bind(ENDPOINT_ID)
    .bind(CREDENTIAL_ID)
    .execute(connection)
    .await
    .expect("legacy request log");
}

async fn endpoint_snapshot(connection: &mut SqliteConnection) -> EndpointSnapshot {
    sqlx::query_as(
        "SELECT id, name, name_key, provider_kind, base_url, protocol_dialect, \
         upstream_protocol_dialect, enabled, config_version, created_at, updated_at \
         FROM provider_endpoints WHERE id = ?",
    )
    .bind(ENDPOINT_ID)
    .fetch_one(connection)
    .await
    .expect("endpoint snapshot")
}

async fn related_row_count(connection: &mut SqliteConnection) -> i64 {
    sqlx::query_scalar(
        "SELECT \
         (SELECT COUNT(*) FROM provider_credentials WHERE provider_endpoint_id = ?) + \
         (SELECT COUNT(*) FROM provider_credential_models WHERE credential_id = ?) + \
         (SELECT COUNT(*) FROM model_routes WHERE id = ?) + \
         (SELECT COUNT(*) FROM route_targets WHERE provider_endpoint_id = ?) + \
         (SELECT COUNT(*) FROM request_logs WHERE provider_endpoint_id = ? AND credential_id = ?)",
    )
    .bind(ENDPOINT_ID)
    .bind(CREDENTIAL_ID)
    .bind(ROUTE_ID)
    .bind(ENDPOINT_ID)
    .bind(ENDPOINT_ID)
    .bind(CREDENTIAL_ID)
    .fetch_one(connection)
    .await
    .expect("related row count")
}
