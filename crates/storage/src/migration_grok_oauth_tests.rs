use std::borrow::Cow;

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::{TempDir, tempdir};

use super::{MIGRATOR, run};

const DIRECT_PROXY_ID: &str = "00000000-0000-0000-0000-000000000000";
const CODEX_ACCOUNT_ID: &str = "10000000-0000-0000-0000-000000000000";
const GROK_ACCOUNT_ID: &str = "20000000-0000-0000-0000-000000000000";
const REQUEST_ID: &str = "30000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn migration_25_preserves_oauth_models_and_request_references() {
    let (_directory, pool) = pool_at_migration_24().await;
    seed_oauth_reference_graph(&pool).await;

    run(&pool).await.expect("upgrade migration 24 database");

    let schema = sqlx::query_scalar::<_, String>(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'oauth_accounts'",
    )
    .fetch_one(&pool)
    .await
    .expect("OAuth account schema");
    assert!(schema.contains("'grok'"));
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM oauth_accounts").await,
        1
    );
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM oauth_account_models").await,
        1
    );
    assert_eq!(scalar(&pool, "SELECT COUNT(*) FROM request_logs").await, 1);
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM request_attempts").await,
        1
    );

    let references = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT l.oauth_account_id, a.oauth_account_id \
         FROM request_logs l JOIN request_attempts a USING (request_id) \
         WHERE l.request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("preserved OAuth references");
    assert_eq!(
        references,
        (Some(CODEX_ACCOUNT_ID.into()), Some(CODEX_ACCOUNT_ID.into()))
    );

    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, \
          account_generation, config_version, requests_per_minute, enabled) \
         VALUES (?, 'grok', 'Grok', 'grok', ?, 1, 1, 1, NULL, 1)",
    )
    .bind(GROK_ACCOUNT_ID)
    .bind(br#"{"type":"grok","access_token":"grok-secret"}"#.as_slice())
    .execute(&pool)
    .await
    .expect("insert Grok OAuth account");

    sqlx::query("DELETE FROM oauth_accounts WHERE id = ?")
        .bind(CODEX_ACCOUNT_ID)
        .execute(&pool)
        .await
        .expect("delete migrated Codex OAuth account");
    assert_eq!(
        scalar(&pool, "SELECT COUNT(*) FROM oauth_account_models").await,
        0
    );
    let references = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "SELECT l.oauth_account_id, a.oauth_account_id \
         FROM request_logs l JOIN request_attempts a USING (request_id) \
         WHERE l.request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("cleared OAuth references");
    assert_eq!(references, (None, None));
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check")
            .is_empty()
    );
}

async fn pool_at_migration_24() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("migration-24.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("migration 24 pool");
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 24)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .expect("apply migrations through version 24");
    (directory, pool)
}

async fn seed_oauth_reference_graph(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO oauth_accounts \
         (id, provider_kind, label, label_key, oauth_json, token_version, \
          account_generation, config_version, requests_per_minute, enabled, \
          safe_account_email, expires_at) \
         VALUES (?, 'codex', 'Codex', 'codex', ?, 2, 3, 4, 60, 1, \
                 'owner@example.com', 1900000000)",
    )
    .bind(CODEX_ACCOUNT_ID)
    .bind(br#"{"type":"codex","access_token":"codex-secret"}"#.as_slice())
    .execute(pool)
    .await
    .expect("seed OAuth account");
    sqlx::query(
        "INSERT INTO oauth_account_models (oauth_account_id, upstream_model) \
         VALUES (?, 'gpt-test')",
    )
    .bind(CODEX_ACCOUNT_ID)
    .execute(pool)
    .await
    .expect("seed OAuth model");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          public_model, oauth_account_id, proxy_profile_id, status_code, attempt_count, \
          latency_ms, is_stream) \
         VALUES (?, 1000, 1, 'openai_responses', 'responses', 'gpt-test', ?, ?, \
                 200, 1, 15, 0)",
    )
    .bind(REQUEST_ID)
    .bind(CODEX_ACCOUNT_ID)
    .bind(DIRECT_PROXY_ID)
    .execute(pool)
    .await
    .expect("seed request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, oauth_account_id, proxy_profile_id, started_at_ms, \
          duration_ms, outcome) \
         VALUES (?, 1, ?, ?, 1000, 15, 'success')",
    )
    .bind(REQUEST_ID)
    .bind(CODEX_ACCOUNT_ID)
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
