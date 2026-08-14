use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, ModelAccess, OAuthAccountDraft, OAuthAccountId,
    OAuthProxySelection, ProtocolDialect, ProtocolOperation, ProviderCredentialDraft,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyProfileId, SettingKey,
    SettingValue,
};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, SqliteStore};
use tempfile::tempdir;

use super::RouteInspectionStatus;
use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot, SnapshotStore},
    credential::ProviderApiKeySecret,
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn inspection_uses_compiled_candidates_and_policy_without_runtime_health() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let capabilities = crate::test_support::configuration_capabilities();
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(initial, runtime.as_ref(), capabilities.provider_registry())
            .expect("initial snapshot"),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        Arc::clone(&capabilities),
    )
    .expect("publisher");

    let endpoint_id = ProviderEndpointId::new();
    let endpoint = publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex bridge",
                ProviderKind::Codex,
                "https://api.example.com/v1",
                ProtocolDialect::OpenAiResponses,
                Some(ProtocolDialect::OpenAiChatCompletions),
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint");
    let enabled_id = CredentialId::new();
    let enabled = publisher
        .create_provider_credential(
            endpoint.revision(),
            enabled_id,
            endpoint_id,
            credential_draft("Enabled", true),
            ProviderApiKeySecret::new("sk-enabled-inspection".to_owned()),
        )
        .await
        .expect("enabled credential");
    let disabled_id = CredentialId::new();
    let disabled = publisher
        .create_provider_credential(
            enabled.revision(),
            disabled_id,
            endpoint_id,
            credential_draft("Disabled", false),
            ProviderApiKeySecret::new("sk-disabled-inspection".to_owned()),
        )
        .await
        .expect("disabled credential");
    let enabled_models = publisher
        .set_provider_credential_models(
            disabled.revision(),
            enabled_id,
            1,
            vec!["available-model".to_owned(), "blocked-model".to_owned()],
        )
        .await
        .expect("enabled models");
    let _configured = publisher
        .set_provider_credential_models(
            enabled_models.revision(),
            disabled_id,
            1,
            vec!["disabled-model".to_owned()],
        )
        .await
        .expect("disabled models");
    let oauth = publisher
        .activate_oauth_account(
            OAuthAccountId::new(),
            ProviderKind::Grok,
            OAuthAccountDraft::new("Grok OAuth", None, true).expect("OAuth draft"),
            OAuthProxySelection::Global,
            Some("grok@example.com".to_owned()),
            None,
            vec!["grok-4.5".to_owned()],
            OAuthAccountDocument::new(
                ProviderKind::Grok,
                br#"{"access_token":"access-secret","refresh_token":null,"id_token":null,"account_id":null,"email":"grok@example.com"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth document"),
        )
        .await
        .expect("OAuth account");
    let snapshot = publisher
        .set_setting_override(
            oauth.revision(),
            SettingKey::ModelsAllowed,
            SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
                "available-model".to_owned(),
                "disabled-model".to_owned(),
                "grok-4.5".to_owned(),
            ])),
        )
        .await
        .expect("allowlist");

    let inspection = super::inspect_routes(
        &snapshot,
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
    );

    assert_eq!(inspection.config_revision(), snapshot.revision());
    assert_eq!(inspection.items().len(), 3);
    let available = item(&inspection, "available-model");
    assert!(available.published());
    assert_eq!(available.status(), RouteInspectionStatus::Available);
    let responses = available
        .operations()
        .iter()
        .find(|item| item.operation() == ProtocolOperation::Responses)
        .expect("Responses operation");
    assert_eq!(responses.candidate_groups().len(), 1);
    let group = &responses.candidate_groups()[0];
    assert_eq!(group.provider_kind(), ProviderKind::Codex);
    assert_eq!(group.provider_endpoint_id(), Some(endpoint_id));
    assert_eq!(group.provider_endpoint_name(), Some("Codex bridge"));
    assert_eq!(
        group.upstream_protocol_dialect(),
        ProtocolDialect::OpenAiChatCompletions
    );
    assert_eq!(group.enabled_candidate_count(), 1);
    let compact = available
        .operations()
        .iter()
        .find(|item| item.operation() == ProtocolOperation::ResponsesCompact)
        .expect("compact operation");
    assert!(compact.candidate_groups().is_empty());

    assert!(
        inspection
            .items()
            .iter()
            .all(|item| item.public_model() != "blocked-model")
    );
    let disabled = item(&inspection, "disabled-model");
    assert_eq!(disabled.status(), RouteInspectionStatus::NoEnabledCandidate);
    assert!(
        disabled
            .operations()
            .iter()
            .all(|operation| operation.candidate_groups().is_empty())
    );

    let oauth = item(&inspection, "grok-4.5");
    assert_eq!(oauth.status(), RouteInspectionStatus::Available);
    let oauth_group = &oauth
        .operations()
        .iter()
        .find(|item| item.operation() == ProtocolOperation::Responses)
        .expect("OAuth Responses operation")
        .candidate_groups()[0];
    assert_eq!(oauth_group.provider_kind(), ProviderKind::Grok);
    assert_eq!(oauth_group.provider_endpoint_id(), None);
    assert_eq!(oauth_group.provider_endpoint_name(), None);
    assert_eq!(oauth_group.enabled_candidate_count(), 1);
}

fn credential_draft(label: &str, enabled: bool) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        None,
        enabled,
    )
    .expect("credential draft")
}

fn item<'a>(
    inspection: &'a super::RouteInspectionSnapshot,
    model: &str,
) -> &'a super::RouteInspectionItem {
    inspection
        .items()
        .iter()
        .find(|item| item.public_model() == model)
        .expect("inspection item")
}
