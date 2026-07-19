use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use any2api_domain::{
    CredentialId, ModelRoute, ProviderEndpointId, ProxyProfileId, RouteTargetId, TransportMode,
};
use any2api_provider::api::ProviderRegistry;

use crate::health::{AttemptHealth, HealthAcquireError};
use crate::health::{EndpointHealthRuntime, ProxyHealthRuntime, ReliabilityPolicy};
use crate::{credential_runtime::CredentialRuntimeBinding, published_snapshot::PublishedSnapshot};

#[derive(Clone, Debug)]
pub(crate) struct RouteCandidate {
    pub(crate) target_id: RouteTargetId,
    pub(crate) endpoint_id: ProviderEndpointId,
    pub(crate) credential_id: CredentialId,
    pub(crate) upstream_model: String,
    pub(crate) proxy_id: ProxyProfileId,
    pub(crate) endpoint_health: Option<Arc<EndpointHealthRuntime>>,
    pub(crate) proxy_health: Option<Arc<ProxyHealthRuntime>>,
    pub(crate) binding: CredentialRuntimeBinding,
}

impl RouteCandidate {
    pub(crate) fn health_availability(
        &self,
        policy: &ReliabilityPolicy,
    ) -> Result<(), HealthAcquireError> {
        self.binding
            .generation()
            .health()
            .availability(&self.upstream_model)?;
        if let Some(endpoint) = &self.endpoint_health {
            endpoint.availability(policy)?;
        }
        if let Some(proxy) = &self.proxy_health {
            proxy.availability(policy)?;
        }
        Ok(())
    }

    pub(crate) fn acquire_health(
        &self,
        policy: ReliabilityPolicy,
    ) -> Result<AttemptHealth, HealthAcquireError> {
        self.binding
            .generation()
            .health()
            .availability(&self.upstream_model)?;
        let endpoint = match &self.endpoint_health {
            Some(endpoint) => Some(endpoint.try_acquire(&policy)?),
            None => None,
        };
        let proxy = match &self.proxy_health {
            Some(proxy) => match proxy.try_acquire(&policy) {
                Ok(proxy) => Some(proxy),
                Err(error) => {
                    drop(endpoint);
                    return Err(error);
                }
            },
            None => None,
        };
        Ok(AttemptHealth::new(
            Arc::clone(self.binding.generation()),
            self.upstream_model.clone(),
            endpoint,
            proxy,
            policy,
        ))
    }
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
            let endpoint_health = snapshot.endpoint_health(endpoint.id()).cloned();
            let proxy_health = snapshot.proxy_health(proxy.id()).cloned();

            tiers
                .entry(target.fallback_tier().get())
                .or_insert_with(Vec::new)
                .push(RouteCandidate {
                    target_id: target.id(),
                    endpoint_id: endpoint.id(),
                    credential_id: credential.id(),
                    upstream_model: target.upstream_model().as_str().to_owned(),
                    proxy_id: proxy.id(),
                    endpoint_health,
                    proxy_health,
                    binding: binding.clone(),
                });
        }
    }
    tiers
}

#[derive(Debug, Default)]
pub(crate) struct CandidateExclusions {
    credentials: HashSet<CredentialId>,
    endpoints: HashSet<ProviderEndpointId>,
    proxies: HashSet<ProxyProfileId>,
}

impl CandidateExclusions {
    pub(crate) fn allows(&self, candidate: &RouteCandidate) -> bool {
        !self.credentials.contains(&candidate.credential_id)
            && !self.endpoints.contains(&candidate.endpoint_id)
            && !self.proxies.contains(&candidate.proxy_id)
    }

    pub(crate) fn exclude_credential(&mut self, id: CredentialId) {
        self.credentials.insert(id);
    }

    pub(crate) fn exclude_endpoint(&mut self, id: ProviderEndpointId) {
        self.endpoints.insert(id);
    }

    pub(crate) fn exclude_proxy(&mut self, id: ProxyProfileId) {
        self.proxies.insert(id);
    }
}
