use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ProtocolDialect, ProtocolOperation, ProviderCredentialModel,
    ProviderEndpointDraft, ProviderEndpointId, ProviderKind, PublicModelName, TransportMode,
};
use any2api_protocol::{
    OpenAiResponsesAdapter,
    api::{ProtocolRegistry, RequestExecutionProfile},
};
use any2api_provider::{CodexDriver, api::ProviderRegistry};

use super::{PublisherFixture, credential_draft, endpoint_draft, publisher_fixture};
use crate::{
    configuration::PublishedSnapshot,
    credential::ProviderApiKeySecret,
    routing::{CandidateRequirements, build_route_candidates},
};

#[tokio::test]
async fn credentials_on_same_endpoint_only_serve_their_selected_models() {
    let PublisherFixture { publisher, .. } = publisher_fixture().await;
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
            vec![ProviderCredentialModel::new("model-first", None).expect("credential model")],
        )
        .await
        .expect("first models");
    let snapshot = publisher
        .set_provider_credential_models(
            first_models.revision(),
            second_id,
            1,
            vec![ProviderCredentialModel::new("model-second", None).expect("credential model")],
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
async fn remote_compaction_excludes_responses_to_chat_bridge_targets() {
    let PublisherFixture {
        publisher,
        capabilities,
        ..
    } = publisher_fixture().await;
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let model = PublicModelName::new("bridge-only-model").expect("public model");

    let endpoint = publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            ProviderEndpointDraft::new(
                "Chat bridge",
                ProviderKind::Kimi,
                "https://api.example.com/v1",
                ProtocolDialect::OpenAiResponses,
                Some(ProtocolDialect::OpenAiChatCompletions),
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint");
    let credential = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft("Bridge credential"),
            ProviderApiKeySecret::new("sk-chat-bridge-key".to_owned()),
        )
        .await
        .expect("credential");
    let snapshot = publisher
        .set_provider_credential_models(
            credential.revision(),
            credential_id,
            1,
            vec![ProviderCredentialModel::new(model.as_str(), None).expect("credential model")],
        )
        .await
        .expect("credential models");
    let route = snapshot
        .model_routes()
        .resolve(ProtocolDialect::OpenAiResponses, &model)
        .expect("derived route");

    let standard = build_route_candidates(
        &snapshot,
        route,
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
        CandidateRequirements::new(
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
            TransportMode::Json,
        ),
    );
    assert_eq!(standard.values().flatten().count(), 1);

    let compact = build_route_candidates(
        &snapshot,
        route,
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
        CandidateRequirements::new(
            ProtocolOperation::Responses,
            RequestExecutionProfile::RemoteCompaction,
            TransportMode::Sse,
        ),
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
        CandidateRequirements::new(
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
            TransportMode::Json,
        ),
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
