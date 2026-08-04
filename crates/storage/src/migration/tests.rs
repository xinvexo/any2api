use std::borrow::Cow;

use sqlx::{
    SqliteConnection,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::tempdir;

use super::{MIGRATOR, run};

mod duplicate_attempt_index;
mod gateway_auth_rejected_logs;
mod http_access_log_capacity;
mod http_access_log_loopback_ips;
mod oauth_account_documents;
mod oauth_quota_snapshots;
mod plaintext_schema;
mod query_indexes;
mod request_usage_aggregate_indexes;

const DIRECT_PROXY_ID: &str = "00000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn full_migration_chain_bootstraps_all_current_invariants() {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("initial.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("SQLite pool");

    let mut connection = pool.acquire().await.expect("migration connection");
    run(&mut connection).await.expect("full migration chain");
    drop(connection);

    let migrations = sqlx::query_as::<_, (i64, String)>(
        "SELECT version, description FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .expect("migration rows");
    assert_eq!(
        migrations,
        vec![
            (1, "initial".to_owned()),
            (2, "drop request log cache write tokens".to_owned()),
            (3, "plaintext local secrets".to_owned()),
            (4, "raw http access log exchange".to_owned()),
            (5, "normalize claude base urls".to_owned()),
            (6, "add telemetry query indexes".to_owned()),
            (7, "bound http access log capacity".to_owned()),
            (8, "drop duplicate request attempt index".to_owned()),
            (9, "normalize http access log loopback ips".to_owned()),
            (10, "optimize request usage aggregates".to_owned()),
            (11, "isolate gateway auth rejected logs".to_owned()),
            (12, "persist oauth quota snapshots".to_owned()),
            (13, "canonicalize oauth account documents".to_owned()),
        ]
    );

    let duplicate_attempt_index: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_schema \
         WHERE type = 'index' AND name = 'request_attempts_request_idx'",
    )
    .fetch_optional(&pool)
    .await
    .expect("final attempt index schema");
    assert_eq!(duplicate_attempt_index, None);

    let revision =
        sqlx::query_scalar::<_, i64>("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(&pool)
            .await
            .expect("initial revision");
    assert_eq!(revision, 1);

    let direct = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT name, kind, enabled, built_in FROM proxy_profiles WHERE id = ?",
    )
    .bind(DIRECT_PROXY_ID)
    .fetch_one(&pool)
    .await
    .expect("DIRECT proxy");
    assert_eq!(direct, ("DIRECT".into(), "direct".into(), 1, 1));
    let global = sqlx::query_scalar::<_, String>(
        "SELECT global_proxy_profile_id FROM proxy_settings WHERE singleton_id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("global proxy");
    assert_eq!(global, DIRECT_PROXY_ID);

    let gateway_schema = table_schema(&pool, "gateway_api_keys").await;
    assert!(gateway_schema.contains("token TEXT NOT NULL"));
    assert!(gateway_schema.contains("a2k_v1_"));
    assert!(!gateway_schema.contains("revoked_at"));

    let endpoint_schema = table_schema(&pool, "provider_endpoints").await;
    assert!(endpoint_schema.contains("'grok'"));
    assert!(endpoint_schema.contains("'openai_images'"));
    assert!(!endpoint_schema.contains("'codex_backend'"));
    let model_route_schema = table_schema(&pool, "model_routes").await;
    assert!(model_route_schema.contains("'openai_images'"));
    let route_target_schema = table_schema(&pool, "route_targets").await;
    assert!(route_target_schema.contains("'openai_images'"));
    let request_log_schema = table_schema(&pool, "request_logs").await;
    assert!(request_log_schema.contains("'openai_images'"));
    assert!(request_log_schema.contains("'images_generations'"));
    assert!(request_log_schema.contains("'images_edits'"));
    assert!(request_log_schema.contains("client_ip TEXT NOT NULL"));
    assert!(!request_log_schema.contains("cache_write_tokens"));
    let oauth_schema = table_schema(&pool, "oauth_accounts").await;
    assert!(oauth_schema.contains("oauth_json BLOB NOT NULL"));
    assert!(oauth_schema.contains("requests_per_minute"));
    assert!(!oauth_schema.contains("max_concurrency"));
    let oauth_quota_schema = table_schema(&pool, "oauth_quota_snapshots").await;
    assert!(oauth_quota_schema.contains("ON DELETE CASCADE"));
    assert!(oauth_quota_schema.contains("length(payload) BETWEEN 2 AND 262144"));
    let provider_credential_schema = table_schema(&pool, "provider_credentials").await;
    assert!(provider_credential_schema.contains("api_key BLOB NOT NULL"));
    let proxy_password_schema = table_schema(&pool, "proxy_passwords").await;
    assert!(proxy_password_schema.contains("password BLOB NOT NULL"));
    let access_log_schema = table_schema(&pool, "http_access_logs").await;
    assert!(access_log_schema.contains("exchange_captured INTEGER NOT NULL"));
    assert!(access_log_schema.contains("request_headers BLOB NOT NULL"));
    assert!(access_log_schema.contains("response_body BLOB NOT NULL"));
    assert!(access_log_schema.contains("exchange_bytes INTEGER NOT NULL"));
    assert!(access_log_schema.contains("gateway_auth_rejected INTEGER NOT NULL"));

    let obsolete_tables = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND (\
         name GLOB '*_v[0-9]*' OR name GLOB '*_grok' OR name GLOB '*_images')",
    )
    .fetch_all(&pool)
    .await
    .expect("obsolete tables");
    assert!(
        obsolete_tables.is_empty(),
        "obsolete tables: {obsolete_tables:?}"
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check")
            .is_empty()
    );
}

#[tokio::test]
async fn raw_http_exchange_migration_preserves_legacy_summary_and_marks_detail_unavailable() {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("access-log-upgrade.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("SQLite pool");
    let mut connection = pool.acquire().await.expect("migration connection");
    migrate_through(&mut connection, 3).await;
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, client_ip, method, path, http_version, \
          status_code, duration_ms, response_bytes, outcome) \
         VALUES ('11111111-1111-4111-8111-111111111111', 1000, 1, '203.0.113.8', 'GET', \
                 '/v1/models', 'HTTP/1.1', 200, 9, 42, 'completed')",
    )
    .execute(&mut *connection)
    .await
    .expect("legacy access log");

    migrate_through(&mut connection, 4).await;

    let migrated = sqlx::query_as::<_, (String, String, i64, Vec<u8>, Vec<u8>)>(
        "SELECT path, uri, exchange_captured, request_headers, response_body \
         FROM http_access_logs WHERE request_id = '11111111-1111-4111-8111-111111111111'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("migrated access log");
    assert_eq!(migrated.0, "/v1/models");
    assert_eq!(migrated.1, "/v1/models");
    assert_eq!(migrated.2, 0);
    assert_eq!(migrated.3, b"[]");
    assert!(migrated.4.is_empty());
    assert_eq!(migration_versions(&mut connection).await, vec![1, 2, 3, 4]);
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

#[tokio::test]
async fn claude_base_url_migration_normalizes_v1_and_preserves_related_data() {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("claude-base-url-upgrade.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("SQLite pool");
    let mut connection = pool.acquire().await.expect("migration connection");
    migrate_through(&mut connection, 4).await;
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, enabled, config_version) \
         VALUES \
         ('10000000-0000-4000-8000-000000000001', 'Claude Legacy', 'claude legacy', 'claude', \
          'https://claude.example/proxy/v1', 'anthropic_messages', 1, 7), \
         ('10000000-0000-4000-8000-000000000002', 'Claude Root', 'claude root', 'claude', \
          'https://root.example/proxy', 'anthropic_messages', 1, 3), \
         ('10000000-0000-4000-8000-000000000003', 'Claude Host V1', 'claude host v1', 'claude', \
          'https://v1', 'anthropic_messages', 1, 2), \
         ('10000000-0000-4000-8000-000000000004', 'Codex V1', 'codex v1', 'codex', \
          'https://codex.example/v1', 'openai_responses', 1, 4)",
    )
    .execute(&mut *connection)
    .await
    .expect("representative endpoints");
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_version, \
          credential_generation, config_version, api_key, fingerprint_version, \
          secret_fingerprint, secret_tail, proxy_profile_id, requests_per_minute, enabled) \
         VALUES ('20000000-0000-4000-8000-000000000001', \
                 '10000000-0000-4000-8000-000000000001', 'Legacy Key', 'legacy key', \
                 'api_key', 2, 5, 9, CAST('sk-migration-secret' AS BLOB), 2, zeroblob(32), \
                 'cret', ?, NULL, 1)",
    )
    .bind(DIRECT_PROXY_ID)
    .execute(&mut *connection)
    .await
    .expect("representative credential");
    sqlx::query(
        "INSERT INTO provider_credential_models (credential_id, upstream_model) \
         VALUES ('20000000-0000-4000-8000-000000000001', 'claude-migration-model')",
    )
    .execute(&mut *connection)
    .await
    .expect("representative model");

    migrate_through(&mut connection, 5).await;

    let legacy = sqlx::query_as::<_, (String, i64)>(
        "SELECT base_url, config_version FROM provider_endpoints \
         WHERE id = '10000000-0000-4000-8000-000000000001'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("normalized Claude endpoint");
    assert_eq!(legacy, ("https://claude.example/proxy".into(), 8));
    let unchanged = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, base_url, config_version FROM provider_endpoints \
         WHERE id <> '10000000-0000-4000-8000-000000000001' ORDER BY id",
    )
    .fetch_all(&mut *connection)
    .await
    .expect("unchanged endpoints");
    assert_eq!(
        unchanged,
        vec![
            (
                "10000000-0000-4000-8000-000000000002".into(),
                "https://root.example/proxy".into(),
                3,
            ),
            (
                "10000000-0000-4000-8000-000000000003".into(),
                "https://v1".into(),
                2,
            ),
            (
                "10000000-0000-4000-8000-000000000004".into(),
                "https://codex.example/v1".into(),
                4,
            ),
        ]
    );
    let credential = sqlx::query_as::<_, (i64, i64, Vec<u8>)>(
        "SELECT credential_generation, config_version, api_key FROM provider_credentials \
         WHERE id = '20000000-0000-4000-8000-000000000001'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("preserved credential");
    assert_eq!(credential, (6, 9, b"sk-migration-secret".to_vec()));
    let model = sqlx::query_scalar::<_, String>(
        "SELECT upstream_model FROM provider_credential_models \
         WHERE credential_id = '20000000-0000-4000-8000-000000000001'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("preserved credential model");
    assert_eq!(model, "claude-migration-model");
    let revision =
        sqlx::query_scalar::<_, i64>("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(&mut *connection)
            .await
            .expect("configuration revision");
    assert_eq!(revision, 2);
    assert_eq!(
        migration_versions(&mut connection).await,
        vec![1, 2, 3, 4, 5]
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
}

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

fn migrator_through(maximum_version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            MIGRATOR
                .iter()
                .filter(|migration| migration.version <= maximum_version)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

async fn migrate_through(connection: &mut SqliteConnection, maximum_version: i64) {
    migrator_through(maximum_version)
        .run_direct(connection)
        .await
        .expect("migration subset");
}

async fn migration_versions(connection: &mut SqliteConnection) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(connection)
        .await
        .expect("migration versions")
}

async fn foreign_key_violations(connection: &mut SqliteConnection) -> Vec<sqlx::sqlite::SqliteRow> {
    sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(connection)
        .await
        .expect("foreign key check")
}

async fn table_schema_on_connection(connection: &mut SqliteConnection, table: &str) -> String {
    sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?")
        .bind(table)
        .fetch_one(connection)
        .await
        .expect("table schema")
}

async fn table_schema(pool: &sqlx::SqlitePool, table: &str) -> String {
    sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?")
        .bind(table)
        .fetch_one(pool)
        .await
        .expect("table schema")
}
