use std::borrow::Cow;

use sqlx::{
    SqlitePool,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::{TempDir, tempdir};

use super::{MIGRATOR, run};

const ENDPOINT_ID: &str = "60000000-0000-0000-0000-000000000000";
const MODEL_ROUTE_ID: &str = "61000000-0000-0000-0000-000000000000";
const ROUTE_TARGET_ID: &str = "62000000-0000-0000-0000-000000000000";
const REQUEST_ID: &str = "63000000-0000-0000-0000-000000000000";
const MIGRATION_29_SHA384: &str = "97c9cd31b9f807034f7822e9feed2a5e3b6f64f8d913fcd9061174218da456ccdf3009d774bc426e3dfb728228ef36f2";

#[tokio::test]
async fn migration_29_accepts_openai_images_values_and_preserves_routes_and_logs() {
    let (_directory, pool) = pool_at_migration_28().await;
    seed_existing_rows(&pool).await;

    let migration = MIGRATOR
        .iter()
        .find(|migration| migration.version == 29)
        .expect("migration 29");
    assert_eq!(migration.description, "openai images protocol");
    assert_eq!(hex(&migration.checksum), MIGRATION_29_SHA384);

    run(&pool).await.expect("upgrade migration 28 database");

    set_images_protocol(&pool, "provider_endpoints", "protocol_dialect", ENDPOINT_ID).await;
    set_images_protocol(&pool, "model_routes", "ingress_protocol", MODEL_ROUTE_ID).await;
    set_images_protocol(
        &pool,
        "route_targets",
        "upstream_protocol_dialect",
        ROUTE_TARGET_ID,
    )
    .await;
    sqlx::query(
        "UPDATE request_logs SET ingress_protocol = 'openai_images', \
         operation = 'images_generations' WHERE request_id = ?",
    )
    .bind(REQUEST_ID)
    .execute(&pool)
    .await
    .expect("Images generations request log values accepted");
    sqlx::query("UPDATE request_logs SET operation = 'images_edits' WHERE request_id = ?")
        .bind(REQUEST_ID)
        .execute(&pool)
        .await
        .expect("Images edits request log value accepted");

    let preserved = sqlx::query_as::<_, (String, String, String)>(
        "SELECT public_model, ingress_protocol, operation FROM request_logs WHERE request_id = ?",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("preserved request log");
    assert_eq!(
        preserved,
        (
            "gpt-image-2".to_owned(),
            "openai_images".to_owned(),
            "images_edits".to_owned()
        )
    );
    let preserved_attempt = sqlx::query_scalar::<_, String>(
        "SELECT route_target_id FROM request_attempts WHERE request_id = ? AND attempt_no = 1",
    )
    .bind(REQUEST_ID)
    .fetch_one(&pool)
    .await
    .expect("preserved request attempt");
    assert_eq!(preserved_attempt, ROUTE_TARGET_ID);
    assert!(
        sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("foreign key check")
            .is_empty()
    );
}

async fn seed_existing_rows(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES (?, 'Images', 'images', 'codex', 'https://api.example.com/v1', \
                 'openai_responses', NULL, 1, 1)",
    )
    .bind(ENDPOINT_ID)
    .execute(pool)
    .await
    .expect("seed Images endpoint");
    sqlx::query(
        "INSERT INTO model_routes \
         (id, public_model, ingress_protocol, fallback_on_rate_limit, enabled, config_version) \
         VALUES (?, 'gpt-image-2', 'openai_responses', NULL, 1, 1)",
    )
    .bind(MODEL_ROUTE_ID)
    .execute(pool)
    .await
    .expect("seed model route");
    sqlx::query(
        "INSERT INTO route_targets \
         (id, model_route_id, provider_endpoint_id, upstream_model, \
          upstream_protocol_dialect, fallback_tier, enabled) \
         VALUES (?, ?, ?, 'gpt-image-2', 'openai_responses', 0, 1)",
    )
    .bind(ROUTE_TARGET_ID)
    .bind(MODEL_ROUTE_ID)
    .bind(ENDPOINT_ID)
    .execute(pool)
    .await
    .expect("seed route target");
    sqlx::query(
        "INSERT INTO request_logs \
         (request_id, started_at_ms, config_revision, ingress_protocol, operation, \
          public_model, status_code, attempt_count, latency_ms, is_stream) \
         VALUES (?, 1000, 1, 'openai_responses', 'responses', 'gpt-image-2', 200, 0, 10, 0)",
    )
    .bind(REQUEST_ID)
    .execute(pool)
    .await
    .expect("seed request log");
    sqlx::query(
        "INSERT INTO request_attempts \
         (request_id, attempt_no, route_target_id, started_at_ms, duration_ms, outcome) \
         VALUES (?, 1, ?, 1000, 10, 'success')",
    )
    .bind(REQUEST_ID)
    .bind(ROUTE_TARGET_ID)
    .execute(pool)
    .await
    .expect("seed request attempt");
}

async fn set_images_protocol(pool: &SqlitePool, table: &str, column: &str, id: &str) {
    let query = format!("UPDATE {table} SET {column} = 'openai_images' WHERE id = ?");
    sqlx::query(&query)
        .bind(id)
        .execute(pool)
        .await
        .expect("Images protocol value accepted");
}

async fn pool_at_migration_28() -> (TempDir, SqlitePool) {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("migration-28.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("migration 28 pool");
    let migrations = MIGRATOR
        .iter()
        .filter(|migration| migration.version <= 28)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
    .run(&pool)
    .await
    .expect("apply migrations through version 28");
    (directory, pool)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
