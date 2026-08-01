use any2api_domain::{ConfigRevision, CredentialId, ProviderEndpointId, ProxyProfileId};
use tempfile::tempdir;

use super::{codex_draft, credential_draft, secret};
use crate::{
    api::{ConfigurationMutation, ConfigurationRepository, SqliteStore},
    configuration::commit_configuration,
    error::StorageError,
};

#[tokio::test]
async fn corrupted_plaintext_credential_fails_configuration_loading() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: codex_draft("Codex Primary", "https://api.example.com"),
        },
    )
    .await
    .expect("create endpoint");
    commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: credential_draft("Primary", ProxyProfileId::DIRECT, None, true),
            api_key: secret("sk-corruption-test"),
        },
    )
    .await
    .expect("create credential");
    sqlx::query(
        "UPDATE provider_credentials SET api_key = CAST('different-api-key' AS BLOB) WHERE id = ?",
    )
    .bind(credential_id.to_string())
    .execute(store.pool())
    .await
    .expect("corrupt API key");

    let error = store
        .load_configuration()
        .await
        .expect_err("corrupt API key must fail");
    assert!(matches!(error, StorageError::CorruptConfiguration));
}
