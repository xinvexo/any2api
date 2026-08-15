use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{foreign_key_violations, migrate_through, migration_versions};

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct CredentialModelSnapshot {
    credential_id: String,
    upstream_model: String,
    public_model: Option<String>,
    created_at: String,
}

#[tokio::test]
async fn public_model_alias_migration_extends_rows_and_enforces_uniqueness() {
    let mut connection = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection");
    migrate_through(&mut connection, 32).await;
    insert_parent_credential(&mut connection, "cred-1", "Primary").await;
    insert_parent_credential(&mut connection, "cred-2", "Secondary").await;
    sqlx::query(
        "INSERT INTO provider_credential_models (credential_id, upstream_model, created_at) \
         VALUES ('cred-1', 'gpt-5.6-sol-ganen', '2026-03-04 05:06:07'), \
                ('cred-1', 'gpt-plain', '2026-03-04 05:06:08')",
    )
    .execute(&mut connection)
    .await
    .expect("legacy credential models");

    migrate_through(&mut connection, 33).await;

    assert_eq!(
        model_snapshots(&mut connection).await,
        vec![
            CredentialModelSnapshot {
                credential_id: "cred-1".into(),
                upstream_model: "gpt-5.6-sol-ganen".into(),
                public_model: None,
                created_at: "2026-03-04 05:06:07".into(),
            },
            CredentialModelSnapshot {
                credential_id: "cred-1".into(),
                upstream_model: "gpt-plain".into(),
                public_model: None,
                created_at: "2026-03-04 05:06:08".into(),
            },
        ]
    );
    assert_eq!(
        migration_versions(&mut connection).await,
        (1..=33).collect::<Vec<_>>()
    );
    assert!(foreign_key_violations(&mut connection).await.is_empty());
    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_schema WHERE type = 'index' \
         AND tbl_name = 'provider_credential_models' \
         AND name LIKE 'provider_credential_models_%' ORDER BY name",
    )
    .fetch_all(&mut connection)
    .await
    .expect("model indexes");
    assert_eq!(
        indexes,
        [
            "provider_credential_models_model_idx",
            "provider_credential_models_public_idx",
        ]
    );

    insert_model(
        &mut connection,
        "cred-1",
        "gpt-5.6-vega-ganen",
        Some("gpt-5.6-vega"),
    )
    .await
    .expect("aliased model");
    insert_model(
        &mut connection,
        "cred-2",
        "gpt-plain",
        Some("gpt-5.6-sol-ganen"),
    )
    .await
    .expect("other credential may reuse names");

    let identity_alias = insert_model(&mut connection, "cred-1", "gpt-echo", Some("gpt-echo"))
        .await
        .expect_err("identity alias must be rejected");
    assert!(
        identity_alias
            .to_string()
            .contains("CHECK constraint failed"),
        "unexpected rejection: {identity_alias}"
    );
    let duplicate_public = insert_model(
        &mut connection,
        "cred-1",
        "gpt-other-upstream",
        Some("gpt-plain"),
    )
    .await
    .expect_err("alias colliding with a plain model must be rejected");
    assert!(
        duplicate_public
            .to_string()
            .contains("UNIQUE constraint failed"),
        "unexpected rejection: {duplicate_public}"
    );
}

async fn insert_parent_credential(
    connection: &mut SqliteConnection,
    credential_id: &str,
    label: &str,
) {
    sqlx::query(
        "INSERT OR IGNORE INTO provider_endpoints \
         (id, name, name_key, provider_kind, base_url, protocol_dialect, enabled, config_version) \
         VALUES ('endpoint-1', 'Ganen', 'ganen', 'codex', 'https://api.example.com', \
                 'openai_responses', 1, 1)",
    )
    .execute(&mut *connection)
    .await
    .expect("parent endpoint");
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_version, \
          credential_generation, config_version, api_key, fingerprint_version, \
          secret_fingerprint, secret_tail, proxy_profile_id, requests_per_minute, enabled) \
         VALUES (?, 'endpoint-1', ?, ?, 'api_key', 1, 1, 1, ?, 2, ?, 'tail', \
                 '00000000-0000-0000-0000-000000000000', NULL, 1)",
    )
    .bind(credential_id)
    .bind(label)
    .bind(label.to_lowercase())
    .bind(b"sk-alias-migration".as_slice())
    .bind([3_u8; 32].as_slice())
    .execute(connection)
    .await
    .expect("parent credential");
}

async fn insert_model(
    connection: &mut SqliteConnection,
    credential_id: &str,
    upstream_model: &str,
    public_model: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO provider_credential_models (credential_id, upstream_model, public_model) \
         VALUES (?, ?, ?)",
    )
    .bind(credential_id)
    .bind(upstream_model)
    .bind(public_model)
    .execute(connection)
    .await
    .map(drop)
}

async fn model_snapshots(connection: &mut SqliteConnection) -> Vec<CredentialModelSnapshot> {
    sqlx::query_as(
        "SELECT credential_id, upstream_model, public_model, created_at \
         FROM provider_credential_models WHERE credential_id = 'cred-1' \
         ORDER BY upstream_model",
    )
    .fetch_all(connection)
    .await
    .expect("credential model snapshots")
}
