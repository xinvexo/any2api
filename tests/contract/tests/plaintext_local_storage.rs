use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyAddress, ProxyDraft, ProxyKind,
    ProxyProfileId,
};
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationRepository, SecretBytes, SqliteStore, StoredConfiguration,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::tempdir;

#[tokio::test]
async fn new_storage_uses_the_plaintext_schema_directly() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("any2api.sqlite3");

    let store = SqliteStore::connect(&database).await.expect("storage");
    let proxy_id = ProxyProfileId::new();
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let proxy = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProxy {
            id: proxy_id,
            draft: ProxyDraft::new(
                "Authenticated",
                ProxyKind::Http,
                ProxyAddress::new("proxy.example.com", 8080).expect("proxy address"),
                true,
            )
            .expect("proxy draft"),
        },
    )
    .await;
    let authenticated = commit_configuration(
        &store,
        proxy.revision(),
        ConfigurationMutation::SetProxyAuthentication {
            id: proxy_id,
            username: "proxy-user".to_owned(),
            password: secret("plaintext-proxy-password"),
        },
    )
    .await;
    let endpoint = commit_configuration(
        &store,
        authenticated.revision(),
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("endpoint draft"),
        },
    )
    .await;
    commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                "Primary",
                CredentialKind::ApiKey,
                proxy_id,
                None,
                true,
            )
            .expect("credential draft"),
            api_key: secret("sk-plaintext-provider"),
        },
    )
    .await;

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(&database))
        .await
        .expect("inspection pool");
    let api_key: Vec<u8> =
        sqlx::query_scalar("SELECT api_key FROM provider_credentials WHERE id = ?")
            .bind(credential_id.to_string())
            .fetch_one(&pool)
            .await
            .expect("stored provider API Key");
    let proxy_password: Vec<u8> =
        sqlx::query_scalar("SELECT password FROM proxy_passwords WHERE proxy_profile_id = ?")
            .bind(proxy_id.to_string())
            .fetch_one(&pool)
            .await
            .expect("stored proxy password");
    assert_eq!(api_key, b"sk-plaintext-provider");
    assert_eq!(proxy_password, b"plaintext-proxy-password");
    store.close().await;
    let provider_schema: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = 'provider_credentials'",
    )
    .fetch_one(&pool)
    .await
    .expect("provider schema");
    assert!(provider_schema.contains("api_key BLOB NOT NULL"));
}

async fn commit_configuration(
    store: &SqliteStore,
    expected: ConfigRevision,
    mutation: ConfigurationMutation,
) -> StoredConfiguration {
    use std::convert::Infallible;

    use any2api_storage::api::{
        ConfigurationTransactionOutcome, ConfigurationTransactionRepository,
    };

    let outcome = <SqliteStore as ConfigurationTransactionRepository<
        StoredConfiguration,
        Infallible,
    >>::transact_configuration(store, expected, mutation, Box::new(Ok))
    .await
    .expect("commit configuration");
    match outcome {
        ConfigurationTransactionOutcome::NoChange => store
            .load_configuration()
            .await
            .expect("load unchanged configuration"),
        ConfigurationTransactionOutcome::Committed(configuration) => configuration,
        ConfigurationTransactionOutcome::Rejected(never) => match never {},
    }
}

#[cfg(unix)]
#[tokio::test]
async fn sqlite_files_are_private_under_a_permissive_parent() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().expect("temporary directory");
    let data = root.path().join("data");
    std::fs::create_dir(&data).expect("data directory");
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755))
        .expect("permissive permissions");
    let database = data.join("any2api.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("storage");

    assert_eq!(mode(&data), 0o700);
    for path in [
        database.clone(),
        sidecar_path(&database, "-wal"),
        sidecar_path(&database, "-shm"),
    ] {
        assert!(path.exists(), "{}", path.display());
        assert_eq!(mode(&path), 0o600, "{}", path.display());
    }
    store.close().await;
}

fn secret(value: &str) -> SecretBytes {
    value.as_bytes().to_vec().into()
}

fn sidecar_path(path: &std::path::Path, suffix: &str) -> std::path::PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    value.into()
}

#[cfg(unix)]
fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .expect("path metadata")
        .permissions()
        .mode()
        & 0o777
}
