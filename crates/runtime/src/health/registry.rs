use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use any2api_domain::{
    ProviderEndpointConfiguration, ProviderEndpointId, ProxyConfiguration, ProxyProfileId,
};

use super::runtime::{EndpointHealthRuntime, ProxyHealthRuntime};
use crate::scheduler_epoch::SchedulerEpoch;

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

#[derive(Debug)]
pub(crate) struct HealthRegistry {
    scheduler_epoch: Arc<SchedulerEpoch>,
    endpoints: RwLock<HashMap<EndpointKey, Arc<EndpointHealthRuntime>>>,
    proxies: RwLock<HashMap<ProxyKey, Arc<ProxyHealthRuntime>>>,
}

impl HealthRegistry {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Self {
        Self {
            scheduler_epoch,
            endpoints: RwLock::new(HashMap::new()),
            proxies: RwLock::new(HashMap::new()),
        }
    }

    pub(crate) fn reconcile(
        &self,
        endpoints: &ProviderEndpointConfiguration,
        runtime_endpoints: &[(ProviderEndpointId, u64)],
        proxies: &ProxyConfiguration,
    ) -> HealthBindings {
        let endpoint_bindings = self.reconcile_endpoints(endpoints, runtime_endpoints);
        let proxy_bindings = self.reconcile_proxies(proxies);
        HealthBindings {
            endpoints: endpoint_bindings,
            proxies: proxy_bindings,
        }
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
}

impl HealthBindings {
    pub(crate) fn endpoint(&self, id: ProviderEndpointId) -> Option<&Arc<EndpointHealthRuntime>> {
        self.endpoints.get(&id)
    }

    pub(crate) fn proxy(&self, id: ProxyProfileId) -> Option<&Arc<ProxyHealthRuntime>> {
        self.proxies.get(&id)
    }
}
