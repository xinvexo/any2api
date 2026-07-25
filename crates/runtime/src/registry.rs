use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use any2api_domain::{CredentialId, ModelRouteConfiguration, RoutingCredentialId};
use tokio::sync::watch;

use crate::{
    affinity::{AffinityPolicy, AffinityRegistry, AffinityRuntimeSnapshot},
    balancing::{BalancingRuntimeSnapshot, snapshot as balancing_snapshot},
    credential_runtime::CredentialRuntimeHandle,
    health::{HealthBindings, HealthRegistry},
    process_lifecycle::ProcessLifecycle,
    queue::QueueCoordinator,
    route_tier_cursor::{RouteTierCursorBindings, RouteTierCursorRegistry},
    routing_credential::{RoutingCredentialSpec, RoutingCredentials},
    scheduler_epoch::SchedulerEpoch,
};

#[derive(Debug)]
pub struct RuntimeRegistry {
    scheduler_epoch: Arc<SchedulerEpoch>,
    affinity: Arc<AffinityRegistry>,
    credentials: RwLock<HashMap<RoutingCredentialId, Arc<CredentialRuntimeHandle>>>,
    route_tier_cursors: RouteTierCursorRegistry,
    queue_coordinator: Arc<QueueCoordinator>,
    health: HealthRegistry,
    lifecycle: ProcessLifecycle,
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeRegistry {
    #[must_use]
    pub fn new() -> Self {
        let lifecycle = ProcessLifecycle::new();
        let scheduler_epoch = SchedulerEpoch::with_lifecycle(lifecycle.clone());
        Self {
            affinity: AffinityRegistry::new(),
            scheduler_epoch: Arc::clone(&scheduler_epoch),
            credentials: RwLock::new(HashMap::new()),
            route_tier_cursors: RouteTierCursorRegistry::default(),
            queue_coordinator: QueueCoordinator::new(Arc::clone(&scheduler_epoch)),
            health: HealthRegistry::new(Arc::clone(&scheduler_epoch)),
            lifecycle,
        }
    }

    #[must_use]
    pub fn lifecycle(&self) -> ProcessLifecycle {
        self.lifecycle.clone()
    }

    #[must_use]
    pub fn queue_waiting_count(&self) -> u32 {
        self.queue_coordinator.waiting_count()
    }

    #[must_use]
    pub fn scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch.current()
    }

    pub fn advance_scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch.advance()
    }

    #[must_use]
    pub fn subscribe_scheduler_epoch(&self) -> watch::Receiver<u64> {
        self.scheduler_epoch.subscribe()
    }

    #[must_use]
    pub fn active_credential_count(&self) -> usize {
        self.credentials
            .read()
            .expect("runtime credential registry lock poisoned")
            .len()
    }

    pub(crate) fn reconcile_configuration(
        &self,
        mut specs: Vec<RoutingCredentialSpec>,
    ) -> RoutingCredentials {
        let mut handles = self
            .credentials
            .write()
            .expect("runtime credential registry lock poisoned");
        let mut active_ids = HashSet::with_capacity(specs.len());
        let mut credentials = Vec::with_capacity(specs.len());

        for mut spec in specs.drain(..) {
            let routing_id = spec.id();
            active_ids.insert(routing_id);
            let generation = spec.take_generation();
            let binding = if let Some(handle) = handles.get(&routing_id).cloned() {
                handle.reconcile(routing_id, spec.requests_per_minute(), generation)
            } else {
                let handle = CredentialRuntimeHandle::new(
                    routing_id,
                    spec.requests_per_minute(),
                    generation,
                    Arc::clone(&self.scheduler_epoch),
                );
                let binding = handle.current_binding();
                handles.insert(routing_id, handle);
                binding
            };
            credentials.push(spec.bind(binding));
        }

        handles.retain(|id, handle| {
            if active_ids.contains(id) {
                true
            } else {
                handle.retire();
                false
            }
        });
        self.affinity.retain_credentials(&active_ids);

        RoutingCredentials::new(credentials)
    }

    #[cfg(test)]
    pub(crate) fn reconcile_provider_configuration_for_test(
        &self,
        configuration: &any2api_domain::ProviderCredentialConfiguration,
        mut auth_materials: crate::credential_auth::CredentialAuthMaterials,
    ) -> crate::credential_runtime::CredentialRuntimeBindings {
        let mut handles = self
            .credentials
            .write()
            .expect("runtime credential registry lock poisoned");
        let mut bindings = Vec::with_capacity(configuration.credentials().len());
        let mut active_ids = HashSet::with_capacity(configuration.credentials().len());
        for credential in configuration.credentials() {
            let id = RoutingCredentialId::provider_credential(credential.id());
            active_ids.insert(id);
            let auth = auth_materials.take_for(credential);
            let generation = crate::credential_runtime::CredentialGenerationDefinition::new(
                credential.credential_generation(),
                credential.secret_version(),
                crate::credential_runtime::CredentialAuthentication::provider_api_key(
                    auth.into_provider_secret(),
                ),
            );
            let binding = if let Some(handle) = handles.get(&id).cloned() {
                handle.reconcile(id, credential.requests_per_minute(), generation)
            } else {
                let handle = CredentialRuntimeHandle::new(
                    id,
                    credential.requests_per_minute(),
                    generation,
                    Arc::clone(&self.scheduler_epoch),
                );
                let binding = handle.current_binding();
                handles.insert(id, handle);
                binding
            };
            bindings.push(binding);
        }
        auth_materials.assert_consumed();
        handles.retain(|id, handle| {
            if active_ids.contains(id) {
                true
            } else {
                handle.retire();
                false
            }
        });
        self.affinity.retain_credentials(&active_ids);
        crate::credential_runtime::CredentialRuntimeBindings::new(bindings)
    }

    pub(crate) fn reconcile_route_tier_cursors(
        &self,
        configuration: &ModelRouteConfiguration,
        runtime_keys: &[(any2api_domain::ModelRouteId, any2api_domain::FallbackTier)],
    ) -> RouteTierCursorBindings {
        self.route_tier_cursors
            .reconcile(configuration, runtime_keys)
    }

    pub(crate) fn reconcile_health(
        &self,
        endpoints: &any2api_domain::ProviderEndpointConfiguration,
        runtime_endpoints: &[(any2api_domain::ProviderEndpointId, u64)],
        proxies: &any2api_domain::ProxyConfiguration,
    ) -> HealthBindings {
        self.health.reconcile(endpoints, runtime_endpoints, proxies)
    }

    pub(crate) fn queue_coordinator(&self) -> Arc<QueueCoordinator> {
        Arc::clone(&self.queue_coordinator)
    }

    pub(crate) fn affinity_registry(&self) -> Arc<AffinityRegistry> {
        Arc::clone(&self.affinity)
    }

    #[must_use]
    pub fn affinity_snapshot(
        &self,
        policy: AffinityPolicy,
        limit: usize,
    ) -> AffinityRuntimeSnapshot {
        self.affinity.snapshot(
            policy.soft_ttl(),
            policy.hard_ttl(),
            policy.fixed_wait_timeout(),
            limit,
        )
    }

    pub fn clear_all_affinity(&self) -> usize {
        self.affinity.clear_all()
    }

    pub fn clear_credential_affinity(&self, credential_id: CredentialId) -> usize {
        self.clear_routing_credential_affinity(RoutingCredentialId::provider_credential(
            credential_id,
        ))
    }

    pub fn clear_routing_credential_affinity(&self, credential_id: RoutingCredentialId) -> usize {
        self.affinity.clear_credential(credential_id)
    }

    #[must_use]
    pub fn balancing_snapshot(
        &self,
        published: &crate::published_snapshot::PublishedSnapshot,
    ) -> BalancingRuntimeSnapshot {
        balancing_snapshot(self, published)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeRegistry;

    #[test]
    fn scheduler_epoch_is_monotonic() {
        let registry = RuntimeRegistry::new();

        assert_eq!(registry.advance_scheduler_epoch(), 1);
        assert_eq!(registry.advance_scheduler_epoch(), 2);
        assert_eq!(registry.scheduler_epoch(), 2);
    }
}
