use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ModelAccess, ProtocolDialect,
    ProviderCredentialDraft, ProviderCredentialModel, ProviderEndpointDraft, ProviderEndpointId,
    ProviderKind, ProxyProfileId, PublicModelName, SettingKey, SettingValue,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationMutation, ConfigurationRepository, SecretBytes, SqliteStore},
    configuration::commit_configuration,
};

#[tokio::test]
async fn selected_models_persist_sorted_and_rebuild_routes() {
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
            api_key: secret("sk-model-persistence"),
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
            models: models(&["gpt-z", "gpt-a"]),
        },
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
            .map(|model| model.upstream_model().as_str())
            .collect::<Vec<_>>(),
        ["gpt-a", "gpt-z"]
    );
    assert_eq!(modeled.model_routes().routes().len(), 2);

    let unchanged = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 2,
            models: models(&["gpt-a", "gpt-z"]),
        },
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
            api_key: secret("sk-before-rotation"),
        },
    )
    .await
    .expect("credential");
    let selected = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 1,
            models: models(&["gpt-5.1-codex"]),
        },
    )
    .await
    .expect("selected model");
    assert_eq!(selected.model_routes().routes().len(), 1);

    let rotated = commit_configuration(
        &store,
        selected.revision(),
        ConfigurationMutation::RotateProviderCredentialSecret {
            id: credential_id,
            expected_config_version: 2,
            expected_secret_version: 1,
            api_key: secret("sk-after-rotation"),
        },
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

#[tokio::test]
async fn removing_the_last_model_source_prunes_the_persisted_allowlist() {
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
            api_key: secret("sk-allowlist-pruning"),
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
            models: models(&["gpt-z", "gpt-a"]),
        },
    )
    .await
    .expect("models");
    let other_id = CredentialId::new();
    let other = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: other_id,
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                "Other",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                None,
                true,
            )
            .expect("other credential draft"),
            api_key: secret("sk-other-model"),
        },
    )
    .await
    .expect("other credential");
    let modeled = commit_configuration(
        &store,
        other.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: other_id,
            expected_config_version: 1,
            models: models(&["gpt-b"]),
        },
    )
    .await
    .expect("other model");
    let allowed = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![any2api_domain::SettingOverrideChange::Set {
                key: SettingKey::ModelsAllowed,
                value: SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
                    "gpt-z".to_owned(),
                    "gpt-a".to_owned(),
                    "gpt-z".to_owned(),
                ])),
            }],
        },
    )
    .await
    .expect("model allowlist");
    assert_eq!(
        allowed.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
            "gpt-a".to_owned(),
            "gpt-z".to_owned(),
        ])))
    );

    let reduced = commit_configuration(
        &store,
        allowed.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 2,
            models: models(&["gpt-z"]),
        },
    )
    .await
    .expect("remove one model source");
    assert_eq!(
        reduced.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
            "gpt-z".to_owned()
        ])))
    );

    let deleted = commit_configuration(
        &store,
        reduced.revision(),
        ConfigurationMutation::DeleteProviderCredential {
            id: credential_id,
            expected_config_version: 3,
        },
    )
    .await
    .expect("delete last source");
    assert_eq!(
        deleted.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(
            ModelAccess::Allowlist(Vec::new())
        ))
    );
    assert!(
        !deleted
            .settings()
            .models()
            .allows(&PublicModelName::new("gpt-b").expect("public model"))
    );

    drop(store);
    let restored = SqliteStore::connect(&database)
        .await
        .expect("reopen")
        .load_configuration()
        .await
        .expect("configuration");
    assert_eq!(
        restored
            .settings()
            .override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(
            ModelAccess::Allowlist(Vec::new())
        ))
    );
}

fn models(names: &[&str]) -> Vec<ProviderCredentialModel> {
    names
        .iter()
        .map(|name| ProviderCredentialModel::new(*name, None).expect("credential model"))
        .collect()
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
