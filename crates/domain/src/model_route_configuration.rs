use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::{
    FallbackTier, ModelRoute, ModelRouteDraft, ModelRouteId, ModelRouteValidationError,
    ProtocolDialect, ProviderCredentialConfiguration, ProviderEndpointConfiguration,
    ProviderEndpointId, PublicModelName, RouteTargetDraft, RouteTargetId, UpstreamModelName,
};
use uuid::Uuid;

const MODEL_ROUTE_NAMESPACE: Uuid = Uuid::from_u128(0xb53f_6ddd_8221_5a8b_9ff0_06d4_2ce1_3c64);
const ROUTE_TARGET_NAMESPACE: Uuid = Uuid::from_u128(0x8354_65cc_8cf9_5fc8_859e_10d8_fc96_71fb);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelRouteConfiguration {
    routes: Vec<ModelRoute>,
}

impl ModelRouteConfiguration {
    pub fn from_credentials(
        credentials: &ProviderCredentialConfiguration,
        endpoints: &ProviderEndpointConfiguration,
    ) -> Result<Self, ModelRouteValidationError> {
        let mut groups =
            BTreeMap::<(ProtocolDialect, UpstreamModelName), BTreeSet<ProviderEndpointId>>::new();
        for credential in credentials.credentials() {
            let endpoint = endpoints.get(credential.provider_endpoint_id()).ok_or(
                ModelRouteValidationError::MissingProviderEndpoint(
                    credential.provider_endpoint_id(),
                ),
            )?;
            for model in credential.models() {
                groups
                    .entry((endpoint.protocol_dialect(), model.clone()))
                    .or_default()
                    .insert(endpoint.id());
            }
        }

        let routes = groups
            .into_iter()
            .map(|((dialect, model), endpoint_ids)| {
                let route_id = derived_route_id(dialect, &model);
                let targets = endpoint_ids
                    .into_iter()
                    .map(|endpoint_id| {
                        RouteTargetDraft::new(
                            derived_target_id(route_id, endpoint_id),
                            endpoint_id,
                            model.as_str(),
                            FallbackTier::default(),
                            true,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let draft = ModelRouteDraft::new(model.as_str(), dialect, None, true, targets)?;
                Ok(ModelRoute::create(route_id, draft))
            })
            .collect::<Result<Vec<_>, ModelRouteValidationError>>()?;
        Self::new(routes, endpoints)
    }

    pub fn new(
        mut routes: Vec<ModelRoute>,
        endpoints: &ProviderEndpointConfiguration,
    ) -> Result<Self, ModelRouteValidationError> {
        let mut route_ids = HashSet::new();
        let mut route_keys = HashMap::new();
        let mut target_ids = HashSet::new();
        for route in &routes {
            if !route_ids.insert(route.id()) {
                return Err(ModelRouteValidationError::DuplicateRouteId);
            }
            if route_keys
                .insert(
                    (route.ingress_protocol(), route.public_model().clone()),
                    route.id(),
                )
                .is_some()
            {
                return Err(ModelRouteValidationError::DuplicatePublicModel);
            }
            for target in route.targets() {
                if !target_ids.insert(target.id()) {
                    return Err(ModelRouteValidationError::ReusedTargetId);
                }
                let endpoint = endpoints.get(target.provider_endpoint_id()).ok_or(
                    ModelRouteValidationError::MissingProviderEndpoint(
                        target.provider_endpoint_id(),
                    ),
                )?;
                if endpoint.protocol_dialect() != route.ingress_protocol() {
                    return Err(ModelRouteValidationError::IncompatibleTargetProtocol(
                        target.provider_endpoint_id(),
                    ));
                }
            }
        }
        routes.sort_by(|left, right| {
            left.ingress_protocol()
                .cmp(&right.ingress_protocol())
                .then_with(|| left.public_model().cmp(right.public_model()))
        });
        Ok(Self { routes })
    }

    #[must_use]
    pub const fn initial() -> Self {
        Self { routes: Vec::new() }
    }

    #[must_use]
    pub fn routes(&self) -> &[ModelRoute] {
        &self.routes
    }

    #[must_use]
    pub fn get(&self, id: ModelRouteId) -> Option<&ModelRoute> {
        self.routes.iter().find(|route| route.id() == id)
    }

    #[must_use]
    pub fn resolve(
        &self,
        ingress_protocol: ProtocolDialect,
        public_model: &PublicModelName,
    ) -> Option<&ModelRoute> {
        self.routes.iter().find(|route| {
            route.ingress_protocol() == ingress_protocol && route.public_model() == public_model
        })
    }

    #[must_use]
    pub fn references_endpoint(&self, endpoint_id: ProviderEndpointId) -> bool {
        self.routes.iter().any(|route| {
            route
                .targets()
                .iter()
                .any(|target| target.provider_endpoint_id() == endpoint_id)
        })
    }
}

fn derived_route_id(dialect: ProtocolDialect, model: &UpstreamModelName) -> ModelRouteId {
    let identity = format!("{}\0{}", dialect.as_str(), model.as_str());
    ModelRouteId::from_uuid(Uuid::new_v5(&MODEL_ROUTE_NAMESPACE, identity.as_bytes()))
}

fn derived_target_id(route_id: ModelRouteId, endpoint_id: ProviderEndpointId) -> RouteTargetId {
    let identity = format!("{route_id}\0{endpoint_id}");
    RouteTargetId::from_uuid(Uuid::new_v5(&ROUTE_TARGET_NAMESPACE, identity.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::ModelRouteConfiguration;
    use crate::{
        FallbackTier, ModelRoute, ModelRouteDraft, ModelRouteId, ModelRouteValidationError,
        ProtocolDialect, ProviderEndpoint, ProviderEndpointConfiguration, ProviderEndpointDraft,
        ProviderEndpointId, ProviderKind, RouteTargetDraft, RouteTargetId,
    };

    #[test]
    fn public_models_are_unique_per_protocol_and_targets_must_match_endpoint_dialect() {
        let codex_id = ProviderEndpointId::new();
        let claude_id = ProviderEndpointId::new();
        let endpoints = ProviderEndpointConfiguration::new(vec![
            endpoint(
                codex_id,
                ProviderKind::Codex,
                ProtocolDialect::OpenAiResponses,
            ),
            endpoint(
                claude_id,
                ProviderKind::Claude,
                ProtocolDialect::AnthropicMessages,
            ),
        ])
        .expect("endpoint configuration");
        let responses = route("shared", ProtocolDialect::OpenAiResponses, codex_id);
        let messages = route("shared", ProtocolDialect::AnthropicMessages, claude_id);
        assert!(
            ModelRouteConfiguration::new(vec![responses.clone(), messages], &endpoints).is_ok()
        );

        assert_eq!(
            ModelRouteConfiguration::new(
                vec![
                    responses,
                    route("shared", ProtocolDialect::OpenAiResponses, codex_id),
                ],
                &endpoints,
            )
            .expect_err("duplicate public model"),
            ModelRouteValidationError::DuplicatePublicModel
        );
        assert!(matches!(
            ModelRouteConfiguration::new(
                vec![route("wrong", ProtocolDialect::OpenAiResponses, claude_id)],
                &endpoints,
            ),
            Err(ModelRouteValidationError::IncompatibleTargetProtocol(id)) if id == claude_id
        ));
    }

    fn endpoint(
        id: ProviderEndpointId,
        kind: ProviderKind,
        dialect: ProtocolDialect,
    ) -> ProviderEndpoint {
        ProviderEndpoint::create(
            id,
            ProviderEndpointDraft::new(
                format!("{kind:?}"),
                kind,
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
        public_model: &str,
        dialect: ProtocolDialect,
        endpoint_id: ProviderEndpointId,
    ) -> ModelRoute {
        ModelRoute::create(
            ModelRouteId::new(),
            ModelRouteDraft::new(
                public_model,
                dialect,
                None,
                true,
                vec![
                    RouteTargetDraft::new(
                        RouteTargetId::new(),
                        endpoint_id,
                        "upstream",
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
