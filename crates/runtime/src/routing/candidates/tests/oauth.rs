use std::sync::Arc;

use any2api_domain::{
    FallbackTier, ModelRoute, ModelRouteConfiguration, ModelRouteDraft, ModelRouteId,
    OAuthAccountDraft, OAuthAccountId, ProtocolDialect, ProtocolOperation, ProviderEndpoint,
    ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    PublicModelName, RouteTargetDraft, RouteTargetId, TransportMode,
};
use any2api_protocol::{
    OpenAiResponsesAdapter,
    api::{ProtocolRegistry, RequestExecutionProfile},
};
use any2api_provider::{GrokDriver, api::ProviderRegistry};
use any2api_storage::api::OAuthAccountDocument;

use super::{PublisherFixture, publisher_fixture};
use crate::routing::{
    CandidateRequirements, OAuthRoute, build_oauth_route_candidates, oauth_route_id,
    resolved_oauth_route_id,
};

#[test]
fn disabled_explicit_route_uses_the_synthetic_oauth_identity() {
    let model = PublicModelName::new("oauth-only-model").expect("public model");
    let explicit_id = ModelRouteId::new();
    let endpoint_id = ProviderEndpointId::new();
    let endpoint = ProviderEndpoint::create(
        endpoint_id,
        ProviderEndpointDraft::new(
            "disabled route endpoint",
            ProviderKind::Codex,
            "https://api.example.com",
            ProtocolDialect::OpenAiResponses,
            None,
            true,
        )
        .expect("endpoint draft"),
    )
    .expect("endpoint");
    let disabled = ModelRoute::create(
        explicit_id,
        ModelRouteDraft::new(
            model.as_str(),
            ProtocolDialect::OpenAiResponses,
            None,
            false,
            vec![
                RouteTargetDraft::new(
                    RouteTargetId::new(),
                    endpoint_id,
                    model.as_str(),
                    ProtocolDialect::OpenAiResponses,
                    FallbackTier::default(),
                    true,
                )
                .expect("target draft"),
            ],
        )
        .expect("disabled route draft"),
    );
    let endpoints =
        ProviderEndpointConfiguration::new(vec![endpoint]).expect("endpoint configuration");
    let routes =
        ModelRouteConfiguration::new(vec![disabled], &endpoints).expect("route configuration");

    let resolved = resolved_oauth_route_id(&routes, ProtocolDialect::OpenAiResponses, &model);
    assert_ne!(resolved, explicit_id);
    assert_eq!(
        resolved,
        oauth_route_id(ProtocolDialect::OpenAiResponses, &model)
    );
}

#[tokio::test]
async fn grok_oauth_routes_responses_but_not_compact() {
    let PublisherFixture { publisher, .. } = publisher_fixture().await;
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
                br#"{"access_token":"access-secret","refresh_token":null,"id_token":null,"account_id":null,"email":"grok@example.com"}"#
                    .to_vec()
                    .into(),
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
        CandidateRequirements::new(
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
            TransportMode::Json,
        ),
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
        CandidateRequirements::new(
            ProtocolOperation::ResponsesCompact,
            RequestExecutionProfile::RemoteCompaction,
            TransportMode::Json,
        ),
    );
    assert!(compact.is_empty());
}
