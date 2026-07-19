use std::collections::BTreeMap;

use any2api_domain::{CredentialId, ModelRoute, ProviderEndpointId, RouteTargetId, TransportMode};
use any2api_provider::api::ProviderRegistry;

use crate::{credential_runtime::CredentialRuntimeBinding, published_snapshot::PublishedSnapshot};

#[derive(Clone, Debug)]
pub(crate) struct RouteCandidate {
    pub(crate) target_id: RouteTargetId,
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) credential_id: CredentialId,
    pub(crate) upstream_model: String,
    pub(crate) binding: CredentialRuntimeBinding,
}

pub(crate) fn build_route_candidates(
    snapshot: &PublishedSnapshot,
    route: &ModelRoute,
    providers: &ProviderRegistry,
    transport_mode: TransportMode,
) -> BTreeMap<u16, Vec<RouteCandidate>> {
    let mut tiers = BTreeMap::new();
    for target in route.targets().iter().filter(|target| target.enabled()) {
        let Some(endpoint) = snapshot
            .provider_endpoints()
            .get(target.provider_endpoint_id())
        else {
            continue;
        };
        if !endpoint.enabled() || endpoint.protocol_dialect() != route.ingress_protocol() {
            continue;
        }
        let Some(driver) = providers.get(endpoint.provider_kind()) else {
            continue;
        };
        let capabilities = driver.capabilities();
        if !capabilities.protocols.contains(&route.ingress_protocol())
            || !capabilities.transport_modes.contains(&transport_mode)
        {
            continue;
        }

        for credential in snapshot
            .provider_credentials()
            .for_endpoint(endpoint.id())
            .filter(|credential| credential.enabled())
            .filter(|credential| {
                capabilities
                    .credential_kinds
                    .contains(&credential.credential_kind())
            })
        {
            let Some(binding) = snapshot.credential_runtime(credential.id()) else {
                continue;
            };
            let Some(proxy) = snapshot.resolved_proxy_for_credential(credential.id()) else {
                continue;
            };
            if !proxy.enabled() {
                continue;
            }

            tiers
                .entry(target.fallback_tier().get())
                .or_insert_with(Vec::new)
                .push(RouteCandidate {
                    target_id: target.id(),
                    endpoint_id: endpoint.id(),
                    credential_id: credential.id(),
                    upstream_model: target.upstream_model().as_str().to_owned(),
                    binding: binding.clone(),
                });
        }
    }
    tiers
}
