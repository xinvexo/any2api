use sqlx::SqliteConnection;

use super::{migrate_through, migration_versions, run, table_schema_on_connection};

#[tokio::test]
async fn empty_secret_tables_upgrade_and_preserve_other_configuration() {
    let mut connection = test_connection("empty-secret-upgrade.sqlite3").await;
    migrate_through(&mut connection, 2).await;
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) \
         VALUES ('admin.remote_enabled', 'true')",
    )
    .execute(&mut connection)
    .await
    .expect("setting override");

    run(&mut connection)
        .await
        .expect("plaintext schema migration");

    assert_eq!(migration_versions(&mut connection).await, vec![1, 2, 3]);
    let value: String = sqlx::query_scalar(
        "SELECT value_json FROM setting_overrides WHERE key = 'admin.remote_enabled'",
    )
    .fetch_one(&mut connection)
    .await
    .expect("preserved setting override");
    assert_eq!(value, "true");
    assert!(
        table_schema_on_connection(&mut connection, "provider_credentials")
            .await
            .contains("api_key BLOB NOT NULL")
    );
}

#[tokio::test]
async fn nonempty_previous_gateway_key_is_rejected_before_schema_change() {
    assert_previous_secret_is_rejected(PreviousSecretKind::GatewayApiKey).await;
}

#[tokio::test]
async fn nonempty_previous_provider_credential_is_rejected_before_schema_change() {
    assert_previous_secret_is_rejected(PreviousSecretKind::ProviderCredential).await;
}

#[tokio::test]
async fn nonempty_previous_proxy_password_is_rejected_before_schema_change() {
    assert_previous_secret_is_rejected(PreviousSecretKind::ProxyPassword).await;
}

#[derive(Clone, Copy)]
enum PreviousSecretKind {
    GatewayApiKey,
    ProviderCredential,
    ProxyPassword,
}

async fn assert_previous_secret_is_rejected(kind: PreviousSecretKind) {
    let mut connection = test_connection("rejected-secret-upgrade.sqlite3").await;
    migrate_through(&mut connection, 2).await;
    insert_previous_secret(&mut connection, kind).await;
    let schema_before = schema_snapshot(&mut connection).await;

    run(&mut connection)
        .await
        .expect_err("nonempty previous secret table must be rejected");

    assert_eq!(migration_versions(&mut connection).await, vec![1, 2]);
    assert_eq!(schema_snapshot(&mut connection).await, schema_before);
    assert_eq!(previous_secret_count(&mut connection, kind).await, 1);
}

async fn insert_previous_secret(connection: &mut SqliteConnection, kind: PreviousSecretKind) {
    match kind {
        PreviousSecretKind::GatewayApiKey => {
            let token = format!("a2k_v1_{}", "A".repeat(43));
            sqlx::query(
                "INSERT INTO gateway_api_keys \
                 (id, name, name_key, token, token_prefix, token_hash, hash_version, hash_key_id, \
                  token_version, config_version, enabled) \
                 VALUES ('11111111-1111-4111-8111-111111111111', 'Existing', 'existing', ?, \
                         'a2k_v1_AAAAAAAA', ?, 1, 'existing-key', 1, 1, 1)",
            )
            .bind(token)
            .bind([7_u8; 32].as_slice())
            .execute(connection)
            .await
            .expect("previous gateway key");
        }
        PreviousSecretKind::ProviderCredential => {
            sqlx::query(
                "INSERT INTO provider_credentials \
                 (id, provider_endpoint_id, label, label_key, credential_kind, \
                  secret_schema_version, secret_version, credential_generation, config_version, \
                  envelope_version, key_id, algorithm, nonce, ciphertext, aad_version, \
                  fingerprint_version, secret_fingerprint, secret_tail, proxy_profile_id, \
                  requests_per_minute, enabled) \
                 VALUES ('22222222-2222-4222-8222-222222222222', \
                         '33333333-3333-4333-8333-333333333333', 'Existing', 'existing', \
                         'api_key', 1, 1, 1, 1, 1, 'existing-key', 'xchacha20poly1305', ?, ?, 1, \
                         1, ?, 'tail', '00000000-0000-0000-0000-000000000000', NULL, 1)",
            )
            .bind([1_u8; 24].as_slice())
            .bind([2_u8; 16].as_slice())
            .bind([3_u8; 32].as_slice())
            .execute(connection)
            .await
            .expect("previous provider credential");
        }
        PreviousSecretKind::ProxyPassword => {
            sqlx::query(
                "INSERT INTO proxy_profiles \
                 (id, name, name_key, kind, host, port, enabled, built_in, config_version, \
                  authentication_version) \
                 VALUES ('44444444-4444-4444-8444-444444444444', 'Existing', 'existing', \
                         'http', 'proxy.example.com', 8080, 1, 0, 1, 1)",
            )
            .execute(&mut *connection)
            .await
            .expect("previous proxy profile");
            sqlx::query(
                "INSERT INTO proxy_passwords \
                 (proxy_profile_id, username, authentication_version, envelope_version, key_id, \
                  algorithm, nonce, ciphertext, aad_version) \
                 VALUES ('44444444-4444-4444-8444-444444444444', 'existing-user', 1, 1, \
                         'existing-key', 'xchacha20poly1305', ?, ?, 1)",
            )
            .bind([4_u8; 24].as_slice())
            .bind([5_u8; 16].as_slice())
            .execute(connection)
            .await
            .expect("previous proxy password");
        }
    }
}

async fn previous_secret_count(connection: &mut SqliteConnection, kind: PreviousSecretKind) -> i64 {
    let statement = match kind {
        PreviousSecretKind::GatewayApiKey => "SELECT COUNT(*) FROM gateway_api_keys",
        PreviousSecretKind::ProviderCredential => "SELECT COUNT(*) FROM provider_credentials",
        PreviousSecretKind::ProxyPassword => "SELECT COUNT(*) FROM proxy_passwords",
    };
    sqlx::query_scalar(statement)
        .fetch_one(connection)
        .await
        .expect("previous secret count")
}

async fn schema_snapshot(
    connection: &mut SqliteConnection,
) -> Vec<(String, String, Option<String>)> {
    sqlx::query_as(
        "SELECT type, name, sql FROM sqlite_schema \
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(connection)
    .await
    .expect("schema snapshot")
}

async fn test_connection(_name: &str) -> SqliteConnection {
    use sqlx::{Connection, sqlite::SqliteConnectOptions};

    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(false),
    )
    .await
    .expect("SQLite connection")
}
