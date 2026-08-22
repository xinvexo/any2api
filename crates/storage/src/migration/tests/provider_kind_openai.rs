use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{foreign_key_violations, migrate_through, migration_versions};

const CODEX_ENDPOINT_ID: &str = "10000000-0000-4000-8000-000000000042";
const OPENAI_ENDPOINT_ID: &str = "20000000-0000-4000-8000-000000000042";

#[tokio::test]
async fn openai_provider_migration_preserves_existing_kind_without_url_inference() {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection");
    migrate_through(&mut connection, 41).await;
    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version, created_at, updated_at) \
         VALUES (?, 'Existing Codex', 'existing codex', 'codex', \
                 'https://api.openai.com/v1', 'openai_responses', NULL, 1, 7, \
                 '2026-01-01 00:00:00', '2026-02-02 00:00:00')",
    )
    .bind(CODEX_ENDPOINT_ID)
    .execute(&mut connection)
    .await
    .expect("existing Codex endpoint");

    migrate_through(&mut connection, 42).await;

    let existing = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT provider_kind, base_url, created_at, config_version \
         FROM provider_endpoints WHERE id = ?",
    )
    .bind(CODEX_ENDPOINT_ID)
    .fetch_one(&mut connection)
    .await
    .expect("preserved endpoint");
    assert_eq!(
        existing,
        (
            "codex".to_owned(),
            "https://api.openai.com/v1".to_owned(),
            "2026-01-01 00:00:00".to_owned(),
            7,
        )
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=42).collect::<Vec<_>>()
    );

    sqlx::query(
        "INSERT INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, \
          upstream_protocol_dialect, enabled, config_version) \
         VALUES (?, 'Standard OpenAI', 'standard openai', 'openai', \
                 'https://api.openai.com/v1', 'openai_responses', NULL, 1, 1)",
    )
    .bind(OPENAI_ENDPOINT_ID)
    .execute(&mut connection)
    .await
    .expect("OpenAI endpoint after migration");
}
