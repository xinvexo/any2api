use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId,
};
use tempfile::tempdir;

use crate::api::{ConfigurationRepository, SecretBytes, SqliteStore};

#[tokio::test]
async fn selected_models_persist_sorted_and_rebuild_routes() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = store
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft())
        .await
        .expect("endpoint");
    let created = store
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            secret("sk-model-persistence"),
        )
        .await
        .expect("credential");
    let modeled = store
        .set_provider_credential_models(
            created.revision(),
            credential_id,
            1,
            vec!["gpt-z".to_owned(), "gpt-a".to_owned()],
        )
        .await
        .expect("set models");
    let credential = modeled
        .provider_credentials()
        .get(credential_id)
        .expect("credential");
    assert_eq!(credential.config_version(), 2);
    assert_eq!(
        credential
            .models()
            .iter()
            .map(|model| model.as_str())
            .collect::<Vec<_>>(),
        ["gpt-a", "gpt-z"]
    );
    assert_eq!(modeled.model_routes().routes().len(), 2);

    let unchanged = store
        .set_provider_credential_models(
            modeled.revision(),
            credential_id,
            2,
            vec!["gpt-a".to_owned(), "gpt-z".to_owned()],
        )
        .await
        .expect("no-op model update");
    assert_eq!(unchanged.revision(), modeled.revision());

    drop(store);
    let restored = SqliteStore::connect(&database)
        .await
        .expect("reopened store")
        .load_configuration()
        .await
        .expect("restored configuration");
    assert_eq!(restored.revision(), modeled.revision());
    assert_eq!(
        restored.provider_credentials(),
        modeled.provider_credentials()
    );
    assert_eq!(restored.model_routes(), modeled.model_routes());
}

#[tokio::test]
async fn rotating_secret_clears_selected_models_and_materialized_routes() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = store
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft())
        .await
        .expect("endpoint");
    let created = store
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            secret("sk-before-rotation"),
        )
        .await
        .expect("credential");
    let selected = store
        .set_provider_credential_models(
            created.revision(),
            credential_id,
            1,
            vec!["gpt-5.1-codex".to_owned()],
        )
        .await
        .expect("selected model");
    assert_eq!(selected.model_routes().routes().len(), 1);

    let rotated = store
        .rotate_provider_credential_secret(
            selected.revision(),
            credential_id,
            2,
            1,
            secret("sk-after-rotation"),
        )
        .await
        .expect("rotated credential");
    let credential = rotated
        .provider_credentials()
        .get(credential_id)
        .expect("rotated credential");
    assert!(credential.models().is_empty());
    assert!(rotated.model_routes().routes().is_empty());
    assert_eq!(credential.secret_version(), 2);
    assert_eq!(
        rotated
            .provider_credential_secrets()
            .get(credential_id)
            .expect("rotated secret")
            .expose_for_test(),
        b"sk-after-rotation"
    );
}

fn credential_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(1).expect("max concurrency"),
        true,
    )
    .expect("credential draft")
}

fn endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com",
        ProtocolDialect::OpenAiResponses,
        true,
    )
    .expect("endpoint draft")
}

fn secret(value: &str) -> SecretBytes {
    value.as_bytes().to_vec().into()
}
