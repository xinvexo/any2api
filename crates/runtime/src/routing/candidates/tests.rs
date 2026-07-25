use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, OAuthAccountDraft, OAuthAccountId,
    ProtocolDialect, ProtocolOperation, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId, ProviderKind, ProxyProfileId, PublicModelName, TransportMode,
};
use any2api_protocol::{OpenAiResponsesAdapter, ProtocolRegistry};
use any2api_provider::{CodexDriver, GrokDriver, api::ProviderRegistry};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, SqliteStore};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot, SnapshotStore},
    credential::ProviderApiKeySecret,
    registry::RuntimeRegistry,
    routing::{OAuthRoute, build_oauth_route_candidates, build_route_candidates, oauth_route_id},
};

#[tokio::test]
async fn credentials_on_same_endpoint_only_serve_their_selected_models() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial,
        runtime.as_ref(),
        crate::test_support::configuration_capabilities().provider_registry(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let endpoint_id = ProviderEndpointId::new();
    let first_id = CredentialId::new();
    let second_id = CredentialId::new();

    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft())
        .await
        .expect("endpoint");
    let first = publisher
        .create_provider_credential(
            endpoint.revision(),
            first_id,
            endpoint_id,
            credential_draft("First"),
            ProviderApiKeySecret::new("sk-first-model-key".to_owned()),
        )
        .await
        .expect("first credential");
    let second = publisher
        .create_provider_credential(
            first.revision(),
            second_id,
            endpoint_id,
            credential_draft("Second"),
            ProviderApiKeySecret::new("sk-second-model-key".to_owned()),
        )
        .await
        .expect("second credential");
    let first_models = publisher
        .set_provider_credential_models(
            second.revision(),
            first_id,
            1,
            vec!["model-first".to_owned()],
        )
        .await
        .expect("first models");
    let snapshot = publisher
        .set_provider_credential_models(
            first_models.revision(),
            second_id,
            1,
            vec!["model-second".to_owned()],
        )
        .await
        .expect("second models");

    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");

    assert_eq!(
        candidates_for(&snapshot, &providers, "model-first"),
        vec![first_id]
    );
    assert_eq!(
        candidates_for(&snapshot, &providers, "model-second"),
        vec![second_id]
    );
}

#[tokio::test]
async fn grok_oauth_routes_responses_but_not_compact() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("config.sqlite3"))
            .await
            .expect("storage"),
    );
    let initial = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        initial,
        runtime.as_ref(),
        crate::test_support::configuration_capabilities().provider_registry(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let account_id = OAuthAccountId::new();
    let snapshot = publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Grok,
            OAuthAccountDraft::new("Grok OAuth", None, true).expect("OAuth draft"),
            Some("grok@example.com".into()),
            None,
            vec!["grok-4.5".into()],
            OAuthAccountDocument::new(
                ProviderKind::Grok,
                br#"{"type":"grok","access_token":"access-secret"}"#.to_vec().into(),
            )
            .expect("Grok OAuth document"),
        )
        .await
        .expect("activate Grok OAuth account");

    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    let model = PublicModelName::new("grok-4.5").expect("public model");
    let route = OAuthRoute::new(
        oauth_route_id(ProtocolDialect::OpenAiResponses, &model),
        ProtocolDialect::OpenAiResponses,
        &model,
    );

    let responses = build_oauth_route_candidates(
        &snapshot,
        route,
        &protocols,
        &providers,
        ProtocolOperation::Responses,
        TransportMode::Json,
    );
    let candidate = responses
        .values()
        .flatten()
        .next()
        .expect("Grok OAuth Responses candidate");
    assert_eq!(candidate.credential_id.oauth_account_id(), Some(account_id));
    assert_eq!(
        candidate.base_url.as_str(),
        "https://cli-chat-proxy.grok.com/v1"
    );

    let compact = build_oauth_route_candidates(
        &snapshot,
        route,
        &protocols,
        &providers,
        ProtocolOperation::ResponsesCompact,
        TransportMode::Json,
    );
    assert!(compact.is_empty());
}

fn candidates_for(
    snapshot: &PublishedSnapshot,
    providers: &ProviderRegistry,
    model: &str,
) -> Vec<CredentialId> {
    let mut protocols = ProtocolRegistry::new();
    protocols
        .register(Arc::new(OpenAiResponsesAdapter::new()))
        .expect("Responses adapter");
    let model = PublicModelName::new(model).expect("public model");
    let route = snapshot
        .model_routes()
        .resolve(ProtocolDialect::OpenAiResponses, &model)
        .expect("derived route");
    let candidates = build_route_candidates(
        snapshot,
        route,
        &protocols,
        providers,
        any2api_domain::ProtocolOperation::Responses,
        TransportMode::Json,
    );
    candidates
        .values()
        .flatten()
        .map(|candidate| {
            candidate
                .credential_id
                .provider_credential_id()
                .expect("API Key candidate")
        })
        .collect()
}

fn endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com/v1",
        ProtocolDialect::OpenAiResponses,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft(label: &str) -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        label,
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        None,
        true,
    )
    .expect("credential draft")
}
