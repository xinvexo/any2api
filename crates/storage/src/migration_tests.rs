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
const MIGRATION_15_SHA384: &str = "72b93c41006d479894e2abee0d11137e5a93bdbba1045394aba724579969941957adc0962dd895e7d216a680295591d3";
const MIGRATION_16_SHA384: &str = "a208bd8d29ca5a5b6d16d43e0be135b304e512b296c09c8c3985aafec80efe9bb18ef1bf930a7faaf0cb7a6a366d1e93";

#[tokio::test]
async fn database_at_migration_16_upgrades_without_losing_api_keys() {
    let (_directory, pool) = pool_at_migration_16().await;
    seed_endpoint(&pool).await;
    seed_credential(&pool, "api-credential", "api_key", Some("tail")).await;
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
    assert_eq!(versions, (1..=17).collect::<Vec<_>>());
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
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check")
            .is_empty()
    );
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
