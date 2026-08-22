use std::collections::BTreeMap;

use any2api_domain::{
    ModelRouteConfiguration, ModelRouteId, ProtocolDialect, ProviderEndpointId, PublicModelName,
    RouteTargetId, UpstreamModelName,
};
use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use uuid::Uuid;

use super::{CandidateRequirements, RouteCandidate, route::path_health};
use crate::configuration::PublishedSnapshot;

#[derive(Clone, Copy)]
pub(crate) struct OAuthRoute<'a> {
    route_id: ModelRouteId,
    ingress_protocol: ProtocolDialect,
    public_model: &'a PublicModelName,
}

impl<'a> OAuthRoute<'a> {
    pub(crate) const fn new(
        route_id: ModelRouteId,
        ingress_protocol: ProtocolDialect,
        public_model: &'a PublicModelName,
    ) -> Self {
        Self {
            route_id,
            ingress_protocol,
            public_model,
        }
    }

    pub(crate) const fn route_id(self) -> ModelRouteId {
        self.route_id
    }
}

pub(crate) fn build_oauth_route_candidates(
    snapshot: &PublishedSnapshot,
    route: OAuthRoute<'_>,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
    requirements: CandidateRequirements,
) -> BTreeMap<u16, Vec<RouteCandidate>> {
    let mut tiers = BTreeMap::new();
    add_oauth_candidates(
        &mut tiers,
        snapshot,
        route,
        protocols,
        providers,
        requirements,
    );
    tiers
}

pub(super) fn add_oauth_candidates(
    tiers: &mut BTreeMap<u16, Vec<RouteCandidate>>,
    snapshot: &PublishedSnapshot,
    route: OAuthRoute<'_>,
    protocols: &ProtocolRegistry,
    providers: &ProviderRegistry,
    requirements: CandidateRequirements,
) {
    let Ok(model) = UpstreamModelName::new(route.public_model.as_str().to_owned()) else {
        return;
    };
    for credential in snapshot
        .routing_credentials()
        .iter()
        .filter(|credential| credential.is_oauth())
        .filter(|credential| credential.routable())
        .filter(|credential| credential.ingress_protocol() == route.ingress_protocol)
        .filter(|credential| credential.supports_model(&model))
    {
        let Some(route_admission) = credential.route_admission().cloned() else {
            continue;
        };
        if requirements.execution_profile().requires_same_dialect()
            && credential.upstream_protocol() != route.ingress_protocol
        {
            continue;
        }
        if !protocols.supports_operation(
            route.ingress_protocol,
            credential.upstream_protocol(),
            requirements.operation(),
        ) {
            continue;
        }
        let Some(driver) = providers.get(credential.provider_kind()) else {
            continue;
        };
        let descriptor = driver.descriptor();
        if !descriptor.supports_oauth_operation(requirements.operation()) {
            continue;
        }
        if !descriptor.supports_protocol(credential.upstream_protocol())
            || !descriptor.supports_transport_mode(requirements.transport_mode())
        {
            continue;
        }
        let Some(proxy) = snapshot.proxies().get(credential.proxy_id()) else {
            continue;
        };
        if !proxy.enabled() {
            continue;
        }
        let target_id = oauth_target_id(
            route.route_id,
            credential.endpoint_id(),
            credential.upstream_protocol(),
        );
        let (egress_path_health, candidate_path_health) =
            path_health(snapshot, target_id, requirements.operation(), credential);
        tiers.entry(0).or_default().push(RouteCandidate {
            target_id,
            operation: requirements.operation(),
            endpoint_id: credential.endpoint_id(),
            endpoint_config_version: credential.endpoint_config_version(),
            credential_id: credential.id(),
            routing_generation: credential.binding().generation().routing_generation(),
            provider_kind: credential.provider_kind(),
            base_url: credential.base_url().clone(),
            upstream_model: model.as_str().to_owned(),
            upstream_protocol_dialect: credential.upstream_protocol(),
            proxy_id: proxy.id(),
            proxy_config_version: credential.proxy_config_version(),
            endpoint_health: snapshot.endpoint_health(credential.endpoint_id()).cloned(),
            proxy_health: snapshot.proxy_health(proxy.id()).cloned(),
            egress_path_health,
            candidate_path_health,
            binding: credential.binding().clone(),
            route_admission,
        });
    }
}

const OAUTH_TARGET_NAMESPACE: Uuid = Uuid::from_u128(0x61ad_9f3e_7da0_5cb5_95af_7fe5_9b67_97b2);

pub(super) fn oauth_target_id(
    route_id: ModelRouteId,
    endpoint_id: ProviderEndpointId,
    dialect: ProtocolDialect,
) -> RouteTargetId {
    let identity = format!("{route_id}\0{endpoint_id}\0{}", dialect.as_str());
    RouteTargetId::from_uuid(Uuid::new_v5(&OAUTH_TARGET_NAMESPACE, identity.as_bytes()))
}

const OAUTH_ROUTE_NAMESPACE: Uuid = Uuid::from_u128(0xf0f3_772f_031a_5a1c_a281_1a90_622b_9088);

pub(crate) fn oauth_route_id(
    dialect: ProtocolDialect,
    public_model: &PublicModelName,
) -> ModelRouteId {
    let identity = format!("{}\0{}", dialect.as_str(), public_model.as_str());
    ModelRouteId::from_uuid(Uuid::new_v5(&OAUTH_ROUTE_NAMESPACE, identity.as_bytes()))
}

pub(crate) fn resolved_oauth_route_id(
    routes: &ModelRouteConfiguration,
    dialect: ProtocolDialect,
    public_model: &PublicModelName,
) -> ModelRouteId {
    routes
        .resolve(dialect, public_model)
        .filter(|route| route.enabled())
        .map_or_else(
            || oauth_route_id(dialect, public_model),
            any2api_domain::ModelRoute::id,
        )
}
