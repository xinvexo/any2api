use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::tempdir;

use super::{DIRECT_PROXY_ID, foreign_key_violations, migrate_through, migration_versions};

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
