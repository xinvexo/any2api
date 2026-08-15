use std::collections::{BTreeMap, HashMap, HashSet, btree_map::Entry};

use crate::{
    FallbackTier, ModelRoute, ModelRouteDraft, ModelRouteId, ModelRouteValidationError,
    ProtocolDialect, ProviderCredentialConfiguration, ProviderEndpoint,
    ProviderEndpointConfiguration, ProviderEndpointId, PublicModelName, RouteTargetDraft,
    RouteTargetId, UpstreamModelName,
};
use uuid::Uuid;

const MODEL_ROUTE_NAMESPACE: Uuid = Uuid::from_u128(0xb53f_6ddd_8221_5a8b_9ff0_06d4_2ce1_3c64);
const ROUTE_TARGET_NAMESPACE: Uuid = Uuid::from_u128(0x8354_65cc_8cf9_5fc8_859e_10d8_fc96_71fb);

/// Upstream model per endpoint for one `(dialect, public model)` route.
type RouteGroups =
    BTreeMap<(ProtocolDialect, PublicModelName), BTreeMap<ProviderEndpointId, UpstreamModelName>>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelRouteConfiguration {
    routes: Vec<ModelRoute>,
}

impl ModelRouteConfiguration {
    pub fn from_credentials(
        credentials: &ProviderCredentialConfiguration,
        endpoints: &ProviderEndpointConfiguration,
    ) -> Result<Self, ModelRouteValidationError> {
        let mut groups = RouteGroups::new();
        let mut published =
            BTreeMap::<(ProviderEndpointId, UpstreamModelName), PublicModelName>::new();
        for credential in credentials.credentials() {
            let endpoint = endpoints.get(credential.provider_endpoint_id()).ok_or(
                ModelRouteValidationError::MissingProviderEndpoint(
                    credential.provider_endpoint_id(),
                ),
            )?;
            for model in credential.models() {
                register_endpoint_model(
                    &mut groups,
                    &mut published,
                    endpoint,
                    model.upstream_model().clone(),
                    model.public_model(),
                )?;
            }
        }

        let routes = groups
            .into_iter()
            .map(|((dialect, public_model), targets_by_endpoint)| {
                let route_id = derived_route_id(dialect, &public_model);
                let targets = targets_by_endpoint
                    .into_iter()
                    .map(|(endpoint_id, upstream_model)| {
                        let upstream_dialect = endpoints
                            .get(endpoint_id)
                            .expect("grouped endpoint is present")
                            .effective_upstream_protocol_dialect();
                        RouteTargetDraft::new(
                            derived_target_id(
                                route_id,
                                endpoint_id,
                                upstream_dialect,
                                &upstream_model,
                            ),
                            endpoint_id,
                            upstream_model.as_str(),
                            upstream_dialect,
                            FallbackTier::default(),
                            true,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let draft =
                    ModelRouteDraft::new(public_model.as_str(), dialect, None, true, targets)?;
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
                if endpoint.protocol_dialect() != route.ingress_protocol()
                    || endpoint.effective_upstream_protocol_dialect()
                        != target.upstream_protocol_dialect()
                {
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

/// The `(upstream → public)` mapping of one endpoint must be a bijection so
/// the outbound body for a public model never depends on which credential of
/// that endpoint is selected.
fn register_endpoint_model(
    groups: &mut RouteGroups,
    published: &mut BTreeMap<(ProviderEndpointId, UpstreamModelName), PublicModelName>,
    endpoint: &ProviderEndpoint,
    upstream_model: UpstreamModelName,
    public_model: PublicModelName,
) -> Result<(), ModelRouteValidationError> {
    match published.entry((endpoint.id(), upstream_model.clone())) {
        Entry::Occupied(existing) if existing.get() != &public_model => {
            return Err(ModelRouteValidationError::ConflictingPublicModel {
                endpoint: endpoint.name().to_owned(),
                upstream_model: upstream_model.as_str().to_owned(),
                first: existing.get().as_str().to_owned(),
                second: public_model.as_str().to_owned(),
            });
        }
        Entry::Occupied(_) => {}
        Entry::Vacant(slot) => {
            slot.insert(public_model.clone());
        }
    }
    match groups
        .entry((endpoint.protocol_dialect(), public_model.clone()))
        .or_default()
        .entry(endpoint.id())
    {
        Entry::Occupied(existing) if existing.get() != &upstream_model => {
            Err(ModelRouteValidationError::ConflictingUpstreamModel {
                endpoint: endpoint.name().to_owned(),
                public_model: public_model.as_str().to_owned(),
                first: existing.get().as_str().to_owned(),
                second: upstream_model.as_str().to_owned(),
            })
        }
        Entry::Occupied(_) => Ok(()),
        Entry::Vacant(slot) => {
            slot.insert(upstream_model);
            Ok(())
        }
    }
}

fn derived_route_id(dialect: ProtocolDialect, model: &PublicModelName) -> ModelRouteId {
    let identity = format!("{}\0{}", dialect.as_str(), model.as_str());
    ModelRouteId::from_uuid(Uuid::new_v5(&MODEL_ROUTE_NAMESPACE, identity.as_bytes()))
}

fn derived_target_id(
    route_id: ModelRouteId,
    endpoint_id: ProviderEndpointId,
    upstream_dialect: ProtocolDialect,
    upstream_model: &UpstreamModelName,
) -> RouteTargetId {
    let identity = format!(
        "{route_id}\0{endpoint_id}\0{}\0{}",
        upstream_dialect.as_str(),
        upstream_model.as_str()
    );
    RouteTargetId::from_uuid(Uuid::new_v5(&ROUTE_TARGET_NAMESPACE, identity.as_bytes()))
}
