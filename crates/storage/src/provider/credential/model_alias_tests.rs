use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ProtocolDialect, ProviderCredentialDraft,
    ProviderCredentialModel, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationMutation, ConfigurationRepository, SecretBytes, SqliteStore},
    configuration::commit_configuration,
};

#[tokio::test]
async fn aliased_models_materialize_public_routes_and_survive_reload() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: endpoint_id,
            draft: endpoint_draft(),
        },
    )
    .await
    .expect("endpoint");
    let created = commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: credential_draft(),
            api_key: secret("sk-alias"),
        },
    )
    .await
    .expect("credential");
    let modeled = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 1,
            models: vec![
                ProviderCredentialModel::new("gpt-5.6-sol-ganen", Some("gpt-5.6-sol".to_owned()))
                    .expect("aliased model"),
            ],
        },
    )
    .await
    .expect("aliased models");

    let routes = modeled.model_routes().routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].public_model().as_str(), "gpt-5.6-sol");
    assert_eq!(routes[0].targets().len(), 1);
    assert_eq!(
        routes[0].targets()[0].upstream_model().as_str(),
        "gpt-5.6-sol-ganen"
    );

    let conflict = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: CredentialId::new(),
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                "Conflicting",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                None,
                true,
            )
            .expect("conflicting draft"),
            api_key: secret("sk-conflict"),
        },
    )
    .await
    .expect("conflicting credential");
    let conflicting_id = conflict
        .provider_credentials()
        .credentials()
        .iter()
        .find(|credential| credential.label() == "Conflicting")
        .expect("conflicting credential entry")
        .id();
    let rejected = commit_configuration(
        &store,
        conflict.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: conflicting_id,
            expected_config_version: 1,
            models: vec![ProviderCredentialModel::new("gpt-5.6-sol", None).expect("plain model")],
        },
    )
    .await
    .expect_err("conflicting endpoint mapping must be rejected");
    assert!(
        rejected.to_string().contains("conflicting upstream models"),
        "unexpected rejection: {rejected}"
    );

    drop(store);
    let restored = SqliteStore::connect(&database)
        .await
        .expect("reopened store")
        .load_configuration()
        .await
        .expect("restored configuration");
    let credential = restored
        .provider_credentials()
        .get(credential_id)
        .expect("restored credential");
    assert_eq!(credential.models().len(), 1);
    assert_eq!(
        credential.models()[0]
            .public_alias()
            .expect("persisted alias")
            .as_str(),
        "gpt-5.6-sol"
    );
    assert_eq!(restored.model_routes(), conflict.model_routes());
}

fn credential_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        None,
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
        None,
        true,
    )
    .expect("endpoint draft")
}

fn secret(value: &str) -> SecretBytes {
    value.as_bytes().to_vec().into()
}
