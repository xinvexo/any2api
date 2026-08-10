use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ModelAccess, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId, SettingKey, SettingOverrideChange, SettingValue,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationMutation, SecretBytes, SqliteStore},
    configuration::commit_configuration,
};

#[tokio::test]
async fn deleting_endpoint_cascades_credentials_routes_and_model_allowlist() {
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
            draft: super::codex_draft("https://api.example.com"),
        },
    )
    .await
    .expect("create endpoint");
    let credential = commit_configuration(
        &store,
        endpoint.revision(),
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                "Primary",
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                None,
                true,
            )
            .expect("credential draft"),
            api_key: SecretBytes::from(b"sk-cascade".to_vec()),
        },
    )
    .await
    .expect("create credential");
    let modeled = commit_configuration(
        &store,
        credential.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 1,
            models: vec!["gpt-cascade".to_owned()],
        },
    )
    .await
    .expect("set credential models");
    let allowed = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::ModelsAllowed,
                value: SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
                    "gpt-cascade".to_owned(),
                ])),
            }],
        },
    )
    .await
    .expect("set model allowlist");
    assert_eq!(allowed.model_routes().routes().len(), 1);

    let deleted = commit_configuration(
        &store,
        allowed.revision(),
        ConfigurationMutation::DeleteProviderEndpoint { id: endpoint_id },
    )
    .await
    .expect("delete endpoint with bound credential");

    assert!(deleted.provider_endpoints().get(endpoint_id).is_none());
    assert!(deleted.provider_credentials().credentials().is_empty());
    assert!(deleted.model_routes().routes().is_empty());
    assert_eq!(
        deleted.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(
            ModelAccess::Allowlist(Vec::new())
        ))
    );
    let counts = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT (SELECT COUNT(*) FROM provider_credentials), \
                (SELECT COUNT(*) FROM provider_credential_models), \
                (SELECT COUNT(*) FROM model_routes), \
                (SELECT COUNT(*) FROM route_targets)",
    )
    .fetch_one(store.pool())
    .await
    .expect("query cascaded rows");
    assert_eq!(counts, (0, 0, 0, 0));
}

#[tokio::test]
async fn deleting_endpoint_keeps_a_shared_model_route_target() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let first_endpoint_id = ProviderEndpointId::new();
    let second_endpoint_id = ProviderEndpointId::new();
    let first_credential_id = CredentialId::new();
    let second_credential_id = CredentialId::new();

    let first = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateProviderEndpoint {
            id: first_endpoint_id,
            draft: super::codex_draft("https://first.example.com"),
        },
    )
    .await
    .expect("first endpoint");
    let second = commit_configuration(
        &store,
        first.revision(),
        ConfigurationMutation::CreateProviderEndpoint {
            id: second_endpoint_id,
            draft: ProviderEndpointDraft::new(
                "Second",
                ProviderKind::Codex,
                "https://second.example.com",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("second endpoint draft"),
        },
    )
    .await
    .expect("second endpoint");
    let first_credential = create_credential(
        &store,
        second.revision(),
        first_endpoint_id,
        first_credential_id,
        "Primary",
        b"sk-first",
    )
    .await;
    let second_credential = create_credential(
        &store,
        first_credential.revision(),
        second_endpoint_id,
        second_credential_id,
        "Secondary",
        b"sk-second",
    )
    .await;
    let first_modeled = set_models(&store, &second_credential, first_credential_id).await;
    let modeled = set_models(&store, &first_modeled, second_credential_id).await;

    let deleted = commit_configuration(
        &store,
        modeled.revision(),
        ConfigurationMutation::DeleteProviderEndpoint {
            id: first_endpoint_id,
        },
    )
    .await
    .expect("delete first endpoint");
    let route = deleted
        .model_routes()
        .routes()
        .first()
        .expect("shared route");

    assert_eq!(route.targets().len(), 1);
    assert_eq!(
        route.targets()[0].provider_endpoint_id(),
        second_endpoint_id
    );
    assert!(
        deleted
            .provider_credentials()
            .get(second_credential_id)
            .is_some()
    );
    assert!(
        deleted
            .provider_credentials()
            .get(first_credential_id)
            .is_none()
    );
}

async fn create_credential(
    store: &SqliteStore,
    revision: any2api_domain::ConfigRevision,
    endpoint_id: ProviderEndpointId,
    credential_id: CredentialId,
    label: &str,
    api_key: &[u8],
) -> crate::configuration::StoredConfiguration {
    commit_configuration(
        store,
        revision,
        ConfigurationMutation::CreateProviderCredential {
            id: credential_id,
            endpoint_id,
            draft: ProviderCredentialDraft::new(
                label,
                CredentialKind::ApiKey,
                ProxyProfileId::DIRECT,
                None,
                true,
            )
            .expect("credential draft"),
            api_key: SecretBytes::from(api_key.to_vec()),
        },
    )
    .await
    .expect("credential")
}

async fn set_models(
    store: &SqliteStore,
    current: &crate::configuration::StoredConfiguration,
    credential_id: CredentialId,
) -> crate::configuration::StoredConfiguration {
    commit_configuration(
        store,
        current.revision(),
        ConfigurationMutation::SetProviderCredentialModels {
            id: credential_id,
            expected_config_version: 1,
            models: vec!["gpt-shared".to_owned()],
        },
    )
    .await
    .expect("set shared model")
}
