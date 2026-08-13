use std::borrow::Cow;

use sqlx::{
    SqliteConnection,
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tempfile::tempdir;

use super::{MIGRATOR, run};

mod duplicate_attempt_index;
mod gateway_api_key_prefix;
mod gateway_auth_rejected_logs;
mod http_access_log_capacity;
mod http_access_log_loopback_ips;
mod legacy_upgrades;
mod oauth_account_documents;
mod oauth_quota_estimation_boundaries;
mod oauth_quota_snapshot_v5;
mod oauth_quota_snapshot_v6;
mod oauth_quota_snapshot_v7;
mod oauth_quota_snapshot_v8;
mod oauth_quota_snapshot_v9;
mod oauth_quota_snapshots;
mod plaintext_schema;
mod provider_kind_kimi;
mod query_indexes;
mod request_attempt_routing_diagnostics;
mod request_attempt_transport_stream_diagnostics;
mod request_log_cache_write_tokens;
mod request_usage_aggregate_indexes;
mod response_body_bytes;
mod telemetry_capacity_stats;

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
            (14, "add http access log response body bytes".to_owned()),
            (15, "add telemetry capacity stats".to_owned()),
            (16, "classify request usage by final outcome".to_owned()),
            (17, "add request attempt routing diagnostics".to_owned()),
            (18, "add kimi provider kind".to_owned()),
            (
                19,
                "add request attempt transport stream diagnostics".to_owned()
            ),
            (20, "standard sk gateway api key prefix".to_owned()),
            (21, "version oauth quota snapshot payload".to_owned()),
            (22, "persist oauth quota estimation boundaries".to_owned()),
            (
                23,
                "rebase oauth quota estimates on request logs".to_owned()
            ),
            (
                24,
                "add oauth quota unpriced request diagnostics".to_owned()
            ),
            (25, "epoch interval oauth quota telemetry".to_owned()),
            (26, "monotonic oauth quota telemetry".to_owned()),
            (27, "accumulated segment quota estimator state".to_owned()),
            (28, "single consumer quota estimator state".to_owned()),
            (29, "accumulate codex quota statistics".to_owned()),
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
    assert!(gateway_schema.contains("substr(token, 1, 3) = 'sk-'"));
    assert!(!gateway_schema.contains("revoked_at"));

    let endpoint_schema = table_schema(&pool, "provider_endpoints").await;
    assert!(endpoint_schema.contains("'grok'"));
    assert!(endpoint_schema.contains("'kimi'"));
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
    assert!(request_log_schema.contains("quota_cost_unit TEXT"));
    assert!(request_log_schema.contains("quota_cost_nanos INTEGER"));
    assert!(request_log_schema.contains("quota_cost_rate_card TEXT"));
    assert!(request_log_schema.contains("quota_service_tier TEXT"));
    assert!(request_log_schema.contains("telemetry_process_id TEXT"));
    assert!(request_log_schema.contains("telemetry_sequence INTEGER"));
    let oauth_schema = table_schema(&pool, "oauth_accounts").await;
    assert!(oauth_schema.contains("oauth_json BLOB NOT NULL"));
    assert!(oauth_schema.contains("requests_per_minute"));
    assert!(!oauth_schema.contains("'kimi'"));
    assert!(!oauth_schema.contains("max_concurrency"));
    let oauth_quota_schema = table_schema(&pool, "oauth_quota_snapshots").await;
    assert!(oauth_quota_schema.contains("schema_version = 9"));
    assert!(oauth_quota_schema.contains("ON DELETE CASCADE"));
    assert!(oauth_quota_schema.contains("length(payload) BETWEEN 2 AND 524288"));
    let quota_boundary_exists: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM sqlite_schema WHERE type = 'table' \
         AND name = 'oauth_quota_estimation_boundaries'",
    )
    .fetch_optional(&pool)
    .await
    .expect("removed quota boundary table");
    assert_eq!(quota_boundary_exists, None);
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
    assert!(access_log_schema.contains("response_body_bytes INTEGER NOT NULL"));
    let capacity_stats = sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
        "SELECT request_log_rows, http_access_log_rows, http_access_log_exchange_bytes, \
         gateway_auth_rejected_rows, gateway_auth_rejected_exchange_bytes \
         FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_one(&pool)
    .await
    .expect("telemetry capacity stats singleton");
    assert_eq!(capacity_stats, (0, 0, 0, 0, 0));

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
