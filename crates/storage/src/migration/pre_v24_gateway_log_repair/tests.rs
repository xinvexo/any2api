//! Compatibility repair tests for databases stopped before migration 24.

use std::borrow::Cow;

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::{TempDir, tempdir};

use super::super::{MIGRATOR, run};

const REQUEST_ID: &str = "10000000-0000-0000-0000-000000000000";
const MISSING_GATEWAY_KEY_ID: &str = "20000000-0000-0000-0000-000000000000";
const MISSING_ENDPOINT_ID: &str = "30000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn dangling_gateway_key_log_reference_is_normalized_before_migration_24() {
    let (_directory, pool) = pool_at_migration_23().await;
    insert_dangling_log(&pool, Some(MISSING_GATEWAY_KEY_ID), None).await;

    assert_eq!(foreign_key_violation_count(&pool).await, 1);
    run(&pool)
        .await
        .expect("upgrade database with old telemetry");

    let gateway_key_id = sqlx::query_scalar::<_, Option<String>>(
        "SELECT gateway_api_key_id FROM request_logs WHERE request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("preserved request log");
    assert_eq!(gateway_key_id, None);
    assert_eq!(foreign_key_violation_count(&pool).await, 0);
    assert_eq!(latest_migration(&pool).await, 26);
}

#[tokio::test]
async fn unrelated_foreign_key_damage_still_fails_closed() {
    let (_directory, pool) = pool_at_migration_23().await;
    insert_dangling_log(&pool, None, Some(MISSING_ENDPOINT_ID)).await;

    let error = run(&pool)
        .await
        .expect_err("configuration reference damage must not be repaired");
    assert!(error.to_string().contains("CHECK constraint failed"));
    assert_eq!(latest_migration(&pool).await, 23);
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
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .expect("apply migrations through version 23");
    (directory, pool)
}

async fn insert_dangling_log(
    pool: &SqlitePool,
    gateway_key_id: Option<&str>,
    endpoint_id: Option<&str>,
) {
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(pool)
        .await
        .expect("disable foreign keys for legacy fixture");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, gateway_api_key_id, ingress_protocol, \
          operation, provider_endpoint_id, status_code, attempt_count, latency_ms, is_stream) \
         VALUES (?, 1000, 1, ?, 'openai_responses', 'responses', ?, 200, 0, 10, 0)",
    )
    .bind(REQUEST_ID)
    .bind(gateway_key_id)
    .bind(endpoint_id)
    .execute(pool)
    .await
    .expect("insert legacy dangling log");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await
        .expect("restore foreign keys");
}

async fn foreign_key_violation_count(pool: &SqlitePool) -> usize {
    sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .expect("foreign key check")
        .len()
}

async fn latest_migration(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
        .fetch_one(pool)
        .await
        .expect("latest migration")
}
