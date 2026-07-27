use std::borrow::Cow;

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::{TempDir, tempdir};

use super::{MIGRATOR, run};

const DIRECT_PROXY_ID: &str = "00000000-0000-0000-0000-000000000000";
const ENDPOINT_ID: &str = "10000000-0000-0000-0000-000000000000";
const GROK_ENDPOINT_ID: &str = "20000000-0000-0000-0000-000000000000";
const MIGRATION_15_SHA384: &str = "72b93c41006d479894e2abee0d11137e5a93bdbba1045394aba724579969941957adc0962dd895e7d216a680295591d3";
const MIGRATION_16_SHA384: &str = "a208bd8d29ca5a5b6d16d43e0be135b304e512b296c09c8c3985aafec80efe9bb18ef1bf930a7faaf0cb7a6a366d1e93";
const MIGRATION_27_SHA384: &str = "315a5d3130e2a50de456e376c9c82661d188120f8cb28abbda2147e275758df3586f71677552c397de50862603856604";

#[tokio::test]
async fn database_at_migration_16_upgrades_without_losing_api_keys() {
    let (_directory, pool) = pool_at_migration_16().await;
    seed_endpoint(&pool).await;
    seed_credential(&pool, "api-credential", "api_key", Some("tail")).await;
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) VALUES \
         ('scheduler.on_saturated', '\"reject\"'), \
         ('scheduler.fallback_on_saturation', 'true'), \
         ('scheduler.auxiliary_global_concurrency', '8'), \
         ('scheduler.auxiliary_per_credential_concurrency', '2')",
    )
    .execute(&pool)
    .await
    .expect("seed legacy scheduler overrides");
    sqlx::query(
        "INSERT INTO provider_credential_models (credential_id, upstream_model) VALUES (?, ?)",
    )
    .bind("api-credential")
    .bind("gpt-test")
    .execute(&pool)
    .await
    .expect("seed credential model");

    run(&pool).await.expect("upgrade migration 16 database");

    let versions =
        sqlx::query_scalar::<_, i64>("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("migration versions");
    assert_eq!(versions, (1..=29).collect::<Vec<_>>());
    let kind = sqlx::query_scalar::<_, String>(
        "SELECT credential_kind FROM provider_credentials WHERE id = ?",
    )
    .bind("api-credential")
    .fetch_one(&pool)
    .await
    .expect("preserved API Key credential");
    assert_eq!(kind, "api_key");
    let model = sqlx::query_scalar::<_, String>(
        "SELECT upstream_model FROM provider_credential_models WHERE credential_id = ?",
    )
    .bind("api-credential")
    .fetch_one(&pool)
    .await
    .expect("preserved credential model");
    assert_eq!(model, "gpt-test");
    let schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'provider_credentials'",
    )
    .fetch_one(&pool)
    .await
    .expect("provider credential schema");
    assert!(schema.contains("credential_kind = 'api_key'"));
    assert!(schema.contains("requests_per_minute"));
    assert!(!schema.contains("max_concurrency"));
    let endpoint_schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'provider_endpoints'",
    )
    .fetch_one(&pool)
    .await
    .expect("provider endpoint schema");
    assert!(endpoint_schema.contains("'grok'"));
    let rpm = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT requests_per_minute FROM provider_credentials WHERE id = ?",
    )
    .bind("api-credential")
    .fetch_one(&pool)
    .await
    .expect("migrated optional RPM");
    assert_eq!(rpm, None);
    let oauth_schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'oauth_accounts'",
    )
    .fetch_one(&pool)
    .await
    .expect("OAuth account schema");
    assert!(oauth_schema.contains("oauth_json BLOB NOT NULL"));
    assert!(oauth_schema.contains("proxy_profile_id = '00000000-0000-0000-0000-000000000000'"));
    assert!(oauth_schema.contains("requests_per_minute"));
    assert!(!oauth_schema.contains("max_concurrency"));
    assert!(oauth_schema.contains("'grok'"));
    let setting_keys =
        sqlx::query_scalar::<_, String>("SELECT key FROM setting_overrides ORDER BY key")
            .fetch_all(&pool)
            .await
            .expect("migrated scheduler overrides");
    assert_eq!(
        setting_keys,
        vec![
            "scheduler.fallback_on_rate_limit".to_owned(),
            "scheduler.on_rate_limited".to_owned(),
        ]
    );
    let usage_index = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' \
         AND name = 'request_logs_provider_credential_started_idx'",
    )
    .fetch_one(&pool)
    .await
    .expect("upstream usage index");
    assert_eq!(usage_index, 1);
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES (?, 'Grok', 'grok', 'grok', 'https://api.x.ai/v1', \
                 'openai_responses', NULL, 1, 1)",
    )
    .bind(GROK_ENDPOINT_ID)
    .execute(&pool)
    .await
    .expect("migration 24 accepts Grok provider endpoints");
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, \
          account_generation, config_version, requests_per_minute, enabled) \
         VALUES ('30000000-0000-0000-0000-000000000000', 'grok', 'Grok', 'grok', \
                 x'7b7d', 1, 1, 1, NULL, 1)",
    )
    .execute(&pool)
    .await
    .expect("migration 25 accepts Grok OAuth accounts");
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check")
            .is_empty()
    );
}

#[tokio::test]
async fn migration_26_preserves_existing_logs_with_an_unknown_client_ip() {
    let (_directory, pool) = pool_at_migration_25().await;
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          status_code, attempt_count, latency_ms, is_stream) \
         VALUES ('40000000-0000-0000-0000-000000000000', 1000, 1, \
                 'openai_responses', 'responses', 200, 0, 10, 0)",
    )
    .execute(&pool)
    .await
    .expect("seed migration 25 request log");

    run(&pool).await.expect("upgrade migration 25 database");

    let client_ip = sqlx::query_scalar::<_, Option<String>>(
        "SELECT client_ip FROM request_logs WHERE request_id = \
         '40000000-0000-0000-0000-000000000000'",
    )
    .fetch_one(&pool)
    .await
    .expect("preserved request log");
    assert_eq!(client_ip, None);
    let schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'request_logs'",
    )
    .fetch_one(&pool)
    .await
    .expect("request log schema");
    assert!(schema.contains("client_ip TEXT"));
}

#[tokio::test]
async fn migration_28_preserves_http_logs_and_accepts_long_methods() {
    let (_directory, pool) = pool_at_migration_27().await;
    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, client_ip, method, path, http_version, \
          status_code, duration_ms, response_bytes, outcome) \
         VALUES ('50000000-0000-0000-0000-000000000000', 1000, 1, '127.0.0.1', \
                 'CUSTOM', '/preserved', 'HTTP/1.1', 200, 10, 42, 'completed')",
    )
    .execute(&pool)
    .await
    .expect("seed migration 27 HTTP access log");

    run(&pool).await.expect("upgrade migration 27 database");

    let preserved = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT method, path, response_bytes FROM http_access_logs WHERE request_id = ?",
    )
    .bind("50000000-0000-0000-0000-000000000000")
    .fetch_one(&pool)
    .await
    .expect("preserved HTTP access log");
    assert_eq!(
        preserved,
        ("CUSTOM".to_owned(), "/preserved".to_owned(), 42)
    );

    let schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'http_access_logs'",
    )
    .fetch_one(&pool)
    .await
    .expect("HTTP access log schema");
    assert!(schema.contains("length(method) >= 1"));
    assert!(!schema.contains("length(method) BETWEEN 1 AND 32"));

    sqlx::query(
        "INSERT INTO http_access_logs \
         (request_id, started_at_ms, config_revision, method, path, http_version, duration_ms, \
          response_bytes, outcome) \
         VALUES ('50000000-0000-0000-0000-000000000001', 1001, 1, \
                 'METHOD_WITH_MORE_THAN_THIRTY_TWO_CHARACTERS', '/long-method', 'HTTP/1.1', \
                 1, 0, 'completed')",
    )
    .execute(&pool)
    .await
    .expect("migration 28 accepts a long HTTP method");
    let index_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' \
         AND name = 'http_access_logs_started_idx'",
    )
    .fetch_one(&pool)
    .await
    .expect("HTTP access log index");
    assert_eq!(index_count, 1);
}

#[tokio::test]
async fn legacy_oauth_credentials_block_upgrade_without_deletion() {
    let (_directory, pool) = pool_at_migration_16().await;
    seed_endpoint(&pool).await;
    seed_credential(&pool, "oauth-credential", "oauth2", None).await;

    let error = run(&pool)
        .await
        .expect_err("legacy OAuth credential must block the migration");
    assert!(error.to_string().contains("unsupported_oauth_credentials"));
    let remaining = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM provider_credentials WHERE credential_kind = 'oauth2'",
    )
    .fetch_one(&pool)
    .await
    .expect("legacy OAuth credential count");
    assert_eq!(remaining, 1);
    let applied =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 17")
            .fetch_one(&pool)
            .await
            .expect("migration 17 status");
    assert_eq!(applied, 0);
}

async fn pool_at_migration_16() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("legacy.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("legacy SQLite pool");
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 16)
        .cloned()
        .collect::<Vec<_>>();
    let migration_15 = migrations
        .iter()
        .find(|migration| migration.version == 15)
        .expect("migration 15");
    let migration_16 = migrations
        .iter()
        .find(|migration| migration.version == 16)
        .expect("migration 16");
    assert_eq!(migration_15.description, "provider oauth credentials");
    assert_eq!(hex(&migration_15.checksum), MIGRATION_15_SHA384);
    assert_eq!(migration_16.description, "optional upstream protocol");
    assert_eq!(hex(&migration_16.checksum), MIGRATION_16_SHA384);
    let legacy_migrator = Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    };
    legacy_migrator
        .run(&pool)
        .await
        .expect("apply migrations through version 16");
    (directory, pool)
}

async fn pool_at_migration_25() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("migration-25.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("migration 25 pool");
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 25)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .expect("apply migrations through version 25");
    (directory, pool)
}

async fn pool_at_migration_27() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("migration-27.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("migration 27 pool");
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 27)
        .cloned()
        .collect::<Vec<_>>();
    let migration_27 = migrations
        .iter()
        .find(|migration| migration.version == 27)
        .expect("migration 27");
    assert_eq!(migration_27.description, "http access logs");
    assert_eq!(hex(&migration_27.checksum), MIGRATION_27_SHA384);
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .expect("apply migrations through version 27");
    (directory, pool)
}

async fn seed_endpoint(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES (?, 'Legacy', 'legacy', 'codex', 'https://example.com/v1', \
                 'openai_responses', NULL, 1, 1)",
    )
    .bind(ENDPOINT_ID)
    .execute(pool)
    .await
    .expect("seed endpoint");
}

async fn seed_credential(
    pool: &SqlitePool,
    id: &str,
    credential_kind: &str,
    secret_tail: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_schema_version, \
          secret_version, credential_generation, config_version, envelope_version, key_id, \
          algorithm, nonce, ciphertext, aad_version, fingerprint_version, secret_fingerprint, \
          secret_tail, proxy_profile_id, max_concurrency, enabled) \
         VALUES (?, ?, ?, ?, ?, 1, 1, 1, 1, 1, 'legacy-key', 'xchacha20poly1305', \
                 zeroblob(24), zeroblob(16), 1, 1, zeroblob(32), ?, ?, 1, 1)",
    )
    .bind(id)
    .bind(ENDPOINT_ID)
    .bind(id)
    .bind(id)
    .bind(credential_kind)
    .bind(secret_tail)
    .bind(DIRECT_PROXY_ID)
    .execute(pool)
    .await
    .expect("seed credential");
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
