use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::tempdir;

use super::run;

const DIRECT_PROXY_ID: &str = "00000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn canonical_initial_schema_bootstraps_all_current_invariants() {
    let directory = tempdir().expect("temporary directory");
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("initial.sqlite3"))
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("SQLite pool");

    run(&pool).await.expect("initial migration");

    let migrations = sqlx::query_as::<_, (i64, String)>(
        "SELECT version, description FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .expect("migration rows");
    assert_eq!(migrations, vec![(1, "initial".to_owned())]);

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
    let oauth_schema = table_schema(&pool, "oauth_accounts").await;
    assert!(oauth_schema.contains("oauth_json BLOB NOT NULL"));
    assert!(oauth_schema.contains("requests_per_minute"));
    assert!(!oauth_schema.contains("max_concurrency"));

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

async fn table_schema(pool: &sqlx::SqlitePool, table: &str) -> String {
    sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?")
        .bind(table)
        .fetch_one(pool)
        .await
        .expect("table schema")
}
