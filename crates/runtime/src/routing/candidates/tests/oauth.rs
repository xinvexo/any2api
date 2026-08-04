use std::sync::Arc;

use any2api_domain::{
    OAuthAccountDraft, OAuthAccountId, ProtocolDialect, ProtocolOperation, ProviderKind,
    PublicModelName, TransportMode,
};
use any2api_protocol::{OpenAiResponsesAdapter, ProtocolRegistry, api::RequestExecutionProfile};
use any2api_provider::{GrokDriver, api::ProviderRegistry};
use any2api_storage::api::OAuthAccountDocument;

use super::{PublisherFixture, publisher_fixture};
use crate::routing::{
    CandidateRequirements, OAuthRoute, build_oauth_route_candidates, oauth_route_id,
};

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
