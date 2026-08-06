use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use any2api_domain::{
    ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration, ProxyProfileId,
};

use super::path::{CandidatePathBaseKey, CandidatePathKey, EgressPathKey};
use super::runtime::{EndpointHealthRuntime, ProxyHealthRuntime};
use crate::routing::SchedulerEpoch;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct EndpointKey {
    id: ProviderEndpointId,
    config_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ProxyKey {
    id: ProxyProfileId,
    config_version: u64,
}

#[derive(Debug, Default)]
struct EgressPathHealthState {
    active: HashSet<EgressPathKey>,
    runtimes: HashMap<EgressPathKey, Arc<EndpointHealthRuntime>>,
}

#[derive(Debug, Default)]
struct CandidatePathHealthState {
    active: HashSet<CandidatePathBaseKey>,
    runtimes: HashMap<CandidatePathKey, Arc<EndpointHealthRuntime>>,
}

#[derive(Debug)]
pub(crate) struct HealthRegistry {
    scheduler_epoch: Arc<SchedulerEpoch>,
    endpoints: RwLock<HashMap<EndpointKey, Arc<EndpointHealthRuntime>>>,
    proxies: RwLock<HashMap<ProxyKey, Arc<ProxyHealthRuntime>>>,
    egress_paths: RwLock<EgressPathHealthState>,
    candidate_paths: RwLock<CandidatePathHealthState>,
}

impl HealthRegistry {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Self {
        Self {
            scheduler_epoch,
            endpoints: RwLock::new(HashMap::new()),
            proxies: RwLock::new(HashMap::new()),
            egress_paths: RwLock::new(EgressPathHealthState::default()),
            candidate_paths: RwLock::new(CandidatePathHealthState::default()),
        }
    }

    pub(crate) fn reconcile(
        self: &Arc<Self>,
        endpoints: &ProviderEndpointConfiguration,
        runtime_endpoints: &[(ProviderEndpointId, u64)],
        proxies: &ProxyConfiguration,
        active_candidate_paths: &HashSet<CandidatePathBaseKey>,
    ) -> HealthBindings {
        let endpoint_bindings = self.reconcile_endpoints(endpoints, runtime_endpoints);
        let proxy_bindings = self.reconcile_proxies(proxies);
        self.reconcile_paths(active_candidate_paths);
        HealthBindings {
            endpoints: endpoint_bindings,
            proxies: proxy_bindings,
            registry: Arc::clone(self),
        }
    }

    fn reconcile_paths(&self, active_candidate_paths: &HashSet<CandidatePathBaseKey>) {
        let active_egress_paths = active_candidate_paths
            .iter()
            .map(|path| path.egress_path())
            .collect::<HashSet<_>>();

        let mut egress_paths = self
            .egress_paths
            .write()
            .expect("egress path health registry lock poisoned");
        egress_paths
            .runtimes
            .retain(|key, _| active_egress_paths.contains(key));
        egress_paths.active = active_egress_paths;

        let mut candidate_paths = self
            .candidate_paths
            .write()
            .expect("candidate path health registry lock poisoned");
        candidate_paths
            .runtimes
            .retain(|key, _| active_candidate_paths.contains(&key.base()));
        candidate_paths.active = active_candidate_paths.clone();
    }

    fn egress_path(&self, key: EgressPathKey) -> Arc<EndpointHealthRuntime> {
        if let Some(runtime) = self
            .egress_paths
            .read()
            .expect("egress path health registry lock poisoned")
            .runtimes
            .get(&key)
            .cloned()
        {
            return runtime;
        }
        let mut state = self
            .egress_paths
            .write()
            .expect("egress path health registry lock poisoned");
        if !state.active.contains(&key) {
            return EndpointHealthRuntime::new(Arc::clone(&self.scheduler_epoch));
        }
        state
            .runtimes
            .entry(key)
            .or_insert_with(|| EndpointHealthRuntime::new(Arc::clone(&self.scheduler_epoch)))
            .clone()
    }

    fn candidate_path(&self, key: CandidatePathKey) -> Arc<EndpointHealthRuntime> {
        if let Some(runtime) = self
            .candidate_paths
            .read()
            .expect("candidate path health registry lock poisoned")
            .runtimes
            .get(&key)
            .cloned()
        {
            return runtime;
        }
        let mut state = self
            .candidate_paths
            .write()
            .expect("candidate path health registry lock poisoned");
        if !state.active.contains(&key.base()) {
            return EndpointHealthRuntime::new(Arc::clone(&self.scheduler_epoch));
        }
        state
            .runtimes
            .entry(key)
            .or_insert_with(|| EndpointHealthRuntime::new(Arc::clone(&self.scheduler_epoch)))
            .clone()
    }

    fn reconcile_endpoints(
        &self,
        configuration: &ProviderEndpointConfiguration,
        runtime_endpoints: &[(ProviderEndpointId, u64)],
    ) -> HashMap<ProviderEndpointId, Arc<EndpointHealthRuntime>> {
        let mut active = configuration
            .endpoints()
            .iter()
            .map(|endpoint| EndpointKey {
                id: endpoint.id(),
                config_version: endpoint.config_version(),
            })
            .collect::<HashSet<_>>();
        active.extend(
            runtime_endpoints
                .iter()
                .map(|(id, config_version)| EndpointKey {
                    id: *id,
                    config_version: *config_version,
                }),
        );
        let mut runtimes = self
            .endpoints
            .write()
            .expect("endpoint health registry lock poisoned");
        runtimes.retain(|key, _| active.contains(key));
        active
            .into_iter()
            .map(|key| {
                let runtime = runtimes
                    .entry(key)
                    .or_insert_with(|| {
                        EndpointHealthRuntime::new(Arc::clone(&self.scheduler_epoch))
                    })
                    .clone();
                (key.id, runtime)
            })
            .collect()
    }

    fn reconcile_proxies(
        &self,
        configuration: &ProxyConfiguration,
    ) -> HashMap<ProxyProfileId, Arc<ProxyHealthRuntime>> {
        let active = configuration
            .profiles()
            .iter()
            .map(|proxy| ProxyKey {
                id: proxy.id(),
                config_version: proxy.config_version(),
            })
            .collect::<HashSet<_>>();
        let mut runtimes = self
            .proxies
            .write()
            .expect("proxy health registry lock poisoned");
        runtimes.retain(|key, _| active.contains(key));
        active
            .into_iter()
            .map(|key| {
                let runtime = runtimes
                    .entry(key)
                    .or_insert_with(|| ProxyHealthRuntime::new(Arc::clone(&self.scheduler_epoch)))
                    .clone();
                (key.id, runtime)
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HealthBindings {
    endpoints: HashMap<ProviderEndpointId, Arc<EndpointHealthRuntime>>,
    proxies: HashMap<ProxyProfileId, Arc<ProxyHealthRuntime>>,
    registry: Arc<HealthRegistry>,
}

impl HealthBindings {
    pub(crate) fn endpoint(&self, id: ProviderEndpointId) -> Option<&Arc<EndpointHealthRuntime>> {
        self.endpoints.get(&id)
    }

    pub(crate) fn proxy(&self, id: ProxyProfileId) -> Option<&Arc<ProxyHealthRuntime>> {
        self.proxies.get(&id)
    }

    pub(crate) fn egress_path(&self, key: EgressPathKey) -> Arc<EndpointHealthRuntime> {
        self.registry.egress_path(key)
    }

    pub(crate) fn candidate_path(&self, key: CandidatePathKey) -> Arc<EndpointHealthRuntime> {
        self.registry.candidate_path(key)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use any2api_domain::{
        CredentialId, ProtocolOperation, ProviderEndpointId, ProxyProfileId, RouteTargetId,
        RoutingCredentialId,
    };

    use super::{CandidatePathBaseKey, CandidatePathKey, HealthRegistry};
    use crate::routing::SchedulerEpoch;

    #[test]
    fn late_old_snapshot_paths_are_not_retained_after_reconcile() {
        let registry = HealthRegistry::new(SchedulerEpoch::new());
        let old = path_base(1);
        registry.reconcile_paths(&HashSet::from([old]));
        registry.egress_path(old.egress_path());
        registry.candidate_path(attempt_key(old));
        assert_eq!(registry.egress_paths.read().unwrap().runtimes.len(), 1);
        assert_eq!(registry.candidate_paths.read().unwrap().runtimes.len(), 1);

        let current = path_base(2);
        registry.reconcile_paths(&HashSet::from([current]));
        assert!(registry.egress_paths.read().unwrap().runtimes.is_empty());
        assert!(registry.candidate_paths.read().unwrap().runtimes.is_empty());

        registry.egress_path(old.egress_path());
        registry.candidate_path(attempt_key(old));
        assert!(registry.egress_paths.read().unwrap().runtimes.is_empty());
        assert!(registry.candidate_paths.read().unwrap().runtimes.is_empty());

        registry.egress_path(current.egress_path());
        registry.candidate_path(attempt_key(current));
        assert_eq!(registry.egress_paths.read().unwrap().runtimes.len(), 1);
        assert_eq!(registry.candidate_paths.read().unwrap().runtimes.len(), 1);
    }

    fn path_base(version: u64) -> CandidatePathBaseKey {
        CandidatePathBaseKey {
            target_id: RouteTargetId::new(),
            credential_id: RoutingCredentialId::provider_credential(CredentialId::new()),
            routing_generation: version,
            endpoint_id: ProviderEndpointId::new(),
            endpoint_config_version: version,
            proxy_id: ProxyProfileId::new(),
            proxy_config_version: version,
        }
    }

    fn attempt_key(base: CandidatePathBaseKey) -> CandidatePathKey {
        CandidatePathKey {
            target_id: base.target_id,
            operation: ProtocolOperation::Responses,
            credential_id: base.credential_id,
            routing_generation: base.routing_generation,
            endpoint_id: base.endpoint_id,
            endpoint_config_version: base.endpoint_config_version,
            proxy_id: base.proxy_id,
            proxy_config_version: base.proxy_config_version,
        }
    }
}
