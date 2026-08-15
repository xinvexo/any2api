use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ProtocolDialect, ProtocolOperation, ProviderCredentialModel,
    ProviderEndpointId, PublicModelName, TransportMode,
};
use any2api_protocol::api::RequestExecutionProfile;

use super::{PublisherFixture, credential_draft, endpoint_draft, publisher_fixture};
use crate::{
    credential::ProviderApiKeySecret,
    routing::{CandidateRequirements, OAuthRoute, oauth_route_id},
};

#[tokio::test]
async fn route_candidates_are_cached_per_route_and_requirements() {
    let PublisherFixture {
        publisher,
        capabilities,
        ..
    } = publisher_fixture().await;
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft())
        .await
        .expect("endpoint");
    let credential = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft("Cached"),
            ProviderApiKeySecret::new("sk-cached-model-key".to_owned()),
        )
        .await
        .expect("credential");
    let snapshot = publisher
        .set_provider_credential_models(
            credential.revision(),
            credential_id,
            1,
            vec![ProviderCredentialModel::new("cached-model", None).expect("credential model")],
        )
        .await
        .expect("credential models");
    let model = PublicModelName::new("cached-model").expect("public model");
    let route = snapshot
        .model_routes()
        .resolve(ProtocolDialect::OpenAiResponses, &model)
        .expect("derived route");
    let requirements = CandidateRequirements::new(
        ProtocolOperation::Responses,
        RequestExecutionProfile::Standard,
        TransportMode::Json,
    );

    let first = snapshot.route_candidates(
        route,
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
        requirements,
    );
    let second = snapshot.route_candidates(
        route,
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
        requirements,
    );
    assert_eq!(first.values().flatten().count(), 1);
    assert!(Arc::ptr_eq(&first, &second));

    let streaming = snapshot.route_candidates(
        route,
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
        CandidateRequirements::new(
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
            TransportMode::Sse,
        ),
    );
    assert!(!Arc::ptr_eq(&first, &streaming));

    let entries_before = snapshot.route_candidate_cache_entry_count();
    let unknown = PublicModelName::new("unknown-oauth-model").expect("public model");
    let oauth = snapshot.oauth_route_candidates(
        OAuthRoute::new(
            oauth_route_id(ProtocolDialect::OpenAiResponses, &unknown),
            ProtocolDialect::OpenAiResponses,
            &unknown,
        ),
        capabilities.protocol_registry(),
        capabilities.provider_registry(),
        requirements,
    );
    assert!(oauth.is_empty());
    assert_eq!(snapshot.route_candidate_cache_entry_count(), entries_before);
}
