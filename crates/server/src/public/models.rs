use std::collections::BTreeSet;

use axum::{Json, extract::Extension};
use serde::Serialize;

use super::auth::AuthenticatedGatewayApiKey;

#[derive(Serialize)]
pub(super) struct ModelListResponse {
    object: &'static str,
    data: Vec<ModelListItem>,
}

#[derive(Serialize)]
struct ModelListItem {
    id: String,
    object: &'static str,
    created: u64,
    owned_by: &'static str,
}

pub(super) async fn list_models(
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
) -> Json<ModelListResponse> {
    let data = published_model_names(authenticated.snapshot().model_routes())
        .into_iter()
        .map(|id| ModelListItem {
            id,
            object: "model",
            created: 0,
            owned_by: "any2api",
        })
        .collect();

    Json(ModelListResponse {
        object: "list",
        data,
    })
}

fn published_model_names(routes: &any2api_domain::ModelRouteConfiguration) -> BTreeSet<String> {
    routes
        .routes()
        .iter()
        .filter(|route| route.enabled())
        .map(|route| route.public_model().as_str().to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use any2api_domain::{
        FallbackTier, ModelRoute, ModelRouteConfiguration, ModelRouteDraft, ModelRouteId,
        ProtocolDialect, ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft,
        ProviderEndpointId, ProviderKind, RouteTargetDraft, RouteTargetId,
    };

    use super::published_model_names;

    #[test]
    fn model_list_items_are_sorted_and_cross_protocol_names_are_deduplicated() {
        let codex_endpoint = endpoint(
            ProviderKind::Codex,
            ProtocolDialect::OpenAiResponses,
            "Codex",
        );
        let claude_endpoint = endpoint(
            ProviderKind::Claude,
            ProtocolDialect::AnthropicMessages,
            "Claude",
        );
        let endpoints = ProviderEndpointConfiguration::new(vec![
            codex_endpoint.clone(),
            claude_endpoint.clone(),
        ])
        .expect("endpoints");
        let routes = ModelRouteConfiguration::new(
            vec![
                route(
                    "z-model",
                    ProtocolDialect::OpenAiResponses,
                    codex_endpoint.id(),
                    true,
                ),
                route(
                    "a-model",
                    ProtocolDialect::AnthropicMessages,
                    claude_endpoint.id(),
                    true,
                ),
                route(
                    "a-model",
                    ProtocolDialect::OpenAiResponses,
                    codex_endpoint.id(),
                    false,
                ),
            ],
            &endpoints,
        )
        .expect("routes");

        let names = published_model_names(&routes);
        assert_eq!(
            names.into_iter().collect::<Vec<_>>(),
            ["a-model", "z-model"]
        );
    }

    fn endpoint(
        provider_kind: ProviderKind,
        dialect: ProtocolDialect,
        name: &str,
    ) -> any2api_domain::ProviderEndpoint {
        ProviderEndpoint::create(
            ProviderEndpointId::new(),
            ProviderEndpointDraft::new(
                name,
                provider_kind,
                "https://api.example.com",
                dialect,
                false,
                false,
                true,
            )
            .expect("endpoint draft"),
        )
        .expect("endpoint")
    }

    fn route(
        model: &str,
        dialect: ProtocolDialect,
        endpoint_id: ProviderEndpointId,
        enabled: bool,
    ) -> ModelRoute {
        ModelRoute::create(
            ModelRouteId::new(),
            ModelRouteDraft::new(
                model,
                dialect,
                None,
                enabled,
                vec![
                    RouteTargetDraft::new(
                        RouteTargetId::new(),
                        endpoint_id,
                        model,
                        FallbackTier::default(),
                        true,
                    )
                    .expect("target draft"),
                ],
            )
            .expect("route draft"),
        )
    }
}
