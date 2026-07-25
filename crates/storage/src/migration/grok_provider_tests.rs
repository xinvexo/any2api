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
const CREDENTIAL_ID: &str = "20000000-0000-0000-0000-000000000000";
const ROUTE_ID: &str = "30000000-0000-0000-0000-000000000000";
const TARGET_ID: &str = "40000000-0000-0000-0000-000000000000";
const REQUEST_ID: &str = "50000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn migration_24_preserves_the_complete_provider_reference_graph() {
    let (_directory, pool) = pool_at_migration_23().await;
    seed_reference_graph(&pool).await;

    run(&pool).await.expect("upgrade migration 23 database");

    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM provider_endpoints").await,
        1
    );
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM provider_credentials").await,
        1
    );
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM provider_credential_models").await,
        1
    );
    assert_eq!(scalar(&pool, "SELECT COUNT(*) FROM model_routes").await, 1);
    assert_eq!(scalar(&pool, "SELECT COUNT(*) FROM route_targets").await, 1);
    assert_eq!(scalar(&pool, "SELECT COUNT(*) FROM request_logs").await, 1);
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM request_attempts").await,
        1
    );

    let log = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT provider_endpoint_id, credential_id, error_message, thinking_level \
         FROM request_logs WHERE request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("preserved request log");
    assert_eq!(
        log,
        (
            ENDPOINT_ID.to_owned(),
            CREDENTIAL_ID.to_owned(),
            "upstream diagnostic".to_owned(),
            "high".to_owned(),
        )
    );
    let attempt = sqlx::query_as::<_, (String, String, String)>(
        "SELECT route_target_id, credential_id, error_message \
         FROM request_attempts WHERE request_id = ? AND attempt_no = 1",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("preserved request attempt");
    assert_eq!(
        attempt,
        (
            TARGET_ID.to_owned(),
            CREDENTIAL_ID.to_owned(),
            "attempt diagnostic".to_owned(),
        )
    );
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check")
            .is_empty()
    );
}

async fn pool_at_migration_23() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("migration-23.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("migration 23 pool");
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 23)
        .cloned()
        .collect::<Vec<_>>();
    let legacy_migrator = Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    };
    legacy_migrator
        .run(&pool)
        .await
        .expect("apply migrations through version 23");
    (directory, pool)
}

async fn seed_reference_graph(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES (?, 'Codex', 'codex', 'codex', 'https://api.example.com/v1', \
                 'openai_responses', NULL, 1, 1)",
    )
    .bind(ENDPOINT_ID)
    .execute(pool)
    .await
    .expect("seed endpoint");
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_schema_version, \
          secret_version, credential_generation, config_version, envelope_version, key_id, \
          algorithm, nonce, ciphertext, aad_version, fingerprint_version, secret_fingerprint, \
          secret_tail, proxy_profile_id, enabled, requests_per_minute) \
         VALUES (?, ?, 'Primary', 'primary', 'api_key', 1, 1, 1, 1, 1, 'key-id', \
                 'xchacha20poly1305', zeroblob(24), zeroblob(16), 1, 1, zeroblob(32), \
                 'test', ?, 1, 60)",
    )
    .bind(CREDENTIAL_ID)
    .bind(ENDPOINT_ID)
    .bind(DIRECT_PROXY_ID)
    .execute(pool)
    .await
    .expect("seed credential");
    sqlx::query(
        "INSERT INTO provider_credential_models (credential_id, upstream_model) VALUES (?, 'gpt-test')",
    )
    .bind(CREDENTIAL_ID)
    .execute(pool)
    .await
    .expect("seed credential model");
    sqlx::query(
        "INSERT INTO model_routes \
         (id, public_model, ingress_protocol, fallback_on_rate_limit, enabled, config_version) \
         VALUES (?, 'gpt-test', 'openai_responses', NULL, 1, 1)",
    )
    .bind(ROUTE_ID)
    .execute(pool)
    .await
    .expect("seed model route");
    sqlx::query(
        "INSERT INTO route_targets \
         (id, model_route_id, provider_endpoint_id, upstream_model, \
          upstream_protocol_dialect, fallback_tier, enabled) \
         VALUES (?, ?, ?, 'gpt-test', 'openai_responses', 0, 1)",
    )
    .bind(TARGET_ID)
    .bind(ROUTE_ID)
    .bind(ENDPOINT_ID)
    .execute(pool)
    .await
    .expect("seed route target");
    seed_request_log(pool).await;
}

async fn seed_request_log(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, public_model, \
          provider_endpoint_id, credential_id, proxy_profile_id, status_code, error_class, \
          error_message, attempt_count, latency_ms, first_token_ms, input_tokens, output_tokens, \
          cache_read_tokens, cache_write_tokens, thinking_level, is_stream) \
         VALUES (?, 1000, 1, 'openai_responses', 'responses', 'gpt-test', ?, ?, ?, 503, \
                 'upstream', 'upstream diagnostic', 1, 15, NULL, 10, 2, 1, 0, 'high', 0)",
    )
    .bind(REQUEST_ID)
    .bind(ENDPOINT_ID)
    .bind(CREDENTIAL_ID)
    .bind(DIRECT_PROXY_ID)
    .execute(pool)
    .await
    .expect("seed request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, route_target_id, credential_id, proxy_profile_id, \
          started_at_ms, duration_ms, retry_safety, error_class, error_message, \
          status_code, outcome) \
         VALUES (?, 1, ?, ?, ?, 1000, 15, 'ambiguous', 'upstream', \
                 'attempt diagnostic', 503, 'upstream_error')",
    )
    .bind(REQUEST_ID)
    .bind(TARGET_ID)
    .bind(CREDENTIAL_ID)
    .bind(DIRECT_PROXY_ID)
    .execute(pool)
    .await
    .expect("seed request attempt");
}

async fn scalar(pool: &SqlitePool, query: &str) -> i64 {
    sqlx::query_scalar(query)
        .fetch_one(pool)
        .await
        .expect("scalar query")
}
