use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use any2api_domain::{
    CredentialId, ModelRouteConfiguration, ProviderCredentialConfiguration, SchedulerSettings,
};
use tokio::sync::watch;

use crate::{
    affinity::{AffinityPolicy, AffinityRegistry, AffinityRuntimeSnapshot},
    auxiliary_scheduler::{AuxiliaryConcurrencyLimits, AuxiliaryScheduler},
    credential_auth::CredentialAuthMaterials,
    credential_runtime::{CredentialRuntimeBindings, CredentialRuntimeHandle},
    health::{HealthBindings, HealthRegistry},
    queue::QueueCoordinator,
    route_tier_cursor::{RouteTierCursorBindings, RouteTierCursorRegistry},
    scheduler_epoch::SchedulerEpoch,
};

#[derive(Debug)]
pub struct RuntimeRegistry {
    scheduler_epoch: Arc<SchedulerEpoch>,
    affinity: Arc<AffinityRegistry>,
    credentials: RwLock<HashMap<CredentialId, Arc<CredentialRuntimeHandle>>>,
    route_tier_cursors: RouteTierCursorRegistry,
    auxiliary_scheduler: Arc<AuxiliaryScheduler>,
    queue_coordinator: Arc<QueueCoordinator>,
    health: HealthRegistry,
}

impl RuntimeRegistry {
    #[must_use]
    pub fn new(settings: &SchedulerSettings) -> Self {
        let scheduler_epoch = SchedulerEpoch::new();
        let auxiliary_limits = AuxiliaryConcurrencyLimits::from_scheduler_settings(settings);
        Self {
            affinity: AffinityRegistry::new(),
            auxiliary_scheduler: AuxiliaryScheduler::new(
                auxiliary_limits,
                Arc::clone(&scheduler_epoch),
            ),
            scheduler_epoch: Arc::clone(&scheduler_epoch),
            credentials: RwLock::new(HashMap::new()),
            route_tier_cursors: RouteTierCursorRegistry::default(),
            queue_coordinator: QueueCoordinator::new(Arc::clone(&scheduler_epoch)),
            health: HealthRegistry::new(Arc::clone(&scheduler_epoch)),
        }
    }

    #[must_use]
    pub fn auxiliary_limits(&self) -> AuxiliaryConcurrencyLimits {
        self.auxiliary_scheduler.limits()
    }

    pub(crate) fn reconcile_scheduler_settings(&self, settings: &SchedulerSettings) {
        self.auxiliary_scheduler.reconcile_limits(
            AuxiliaryConcurrencyLimits::from_scheduler_settings(settings),
        );
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
        configuration: &ProviderCredentialConfiguration,
        mut auth_materials: CredentialAuthMaterials,
    ) -> CredentialRuntimeBindings {
        let mut handles = self
            .credentials
            .write()
            .expect("runtime credential registry lock poisoned");
        let mut active_ids = HashSet::with_capacity(configuration.credentials().len());
        let mut bindings = Vec::with_capacity(configuration.credentials().len());

        for credential in configuration.credentials() {
            active_ids.insert(credential.id());
            let auth_material = auth_materials.take_for(credential);
            let binding = if let Some(handle) = handles.get(&credential.id()).cloned() {
                handle.reconcile(credential, auth_material)
            } else {
                let handle = CredentialRuntimeHandle::new(
                    credential,
                    auth_material,
                    Arc::clone(&self.scheduler_epoch),
                );
                let binding = handle.current_binding();
                handles.insert(credential.id(), handle);
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

        CredentialRuntimeBindings::new(bindings)
    }

    pub(crate) fn reconcile_route_tier_cursors(
        &self,
        configuration: &ModelRouteConfiguration,
    ) -> RouteTierCursorBindings {
        self.route_tier_cursors.reconcile(configuration)
    }

    pub(crate) fn reconcile_health(
        &self,
        endpoints: &any2api_domain::ProviderEndpointConfiguration,
        proxies: &any2api_domain::ProxyConfiguration,
    ) -> HealthBindings {
        self.health.reconcile(endpoints, proxies)
    }

    pub(crate) fn auxiliary_scheduler(&self) -> Arc<AuxiliaryScheduler> {
        Arc::clone(&self.auxiliary_scheduler)
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
        self.affinity.clear_credential(credential_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::{SettingKey, SettingOverrides, SettingValue, SettingsConfiguration};

    use super::RuntimeRegistry;

    #[test]
    fn scheduler_epoch_is_monotonic() {
        let settings = SettingsConfiguration::defaults();
        let registry = RuntimeRegistry::new(settings.scheduler());

        assert_eq!(registry.advance_scheduler_epoch(), 1);
        assert_eq!(registry.advance_scheduler_epoch(), 2);
        assert_eq!(registry.scheduler_epoch(), 2);
    }

    #[test]
    fn auxiliary_scheduler_is_stable_when_limits_change() {
        let settings = scheduler_settings(8, 2);
        let registry = RuntimeRegistry::new(settings.scheduler());
        let scheduler = registry.auxiliary_scheduler();

        let updated = scheduler_settings(4, 1);
        registry.reconcile_scheduler_settings(updated.scheduler());

        assert_eq!(registry.auxiliary_limits().global(), 4);
        assert_eq!(registry.auxiliary_limits().per_credential(), 1);
        assert!(Arc::ptr_eq(&scheduler, &registry.auxiliary_scheduler()));
        assert_eq!(registry.scheduler_epoch(), 0);
    }

    fn scheduler_settings(global: u64, per_credential: u64) -> SettingsConfiguration {
        let overrides = SettingOverrides::from_entries([
            (
                SettingKey::SchedulerAuxiliaryGlobalConcurrency,
                SettingValue::Integer(global),
            ),
            (
                SettingKey::SchedulerAuxiliaryPerCredentialConcurrency,
                SettingValue::Integer(per_credential),
            ),
        ])
        .expect("valid settings");
        SettingsConfiguration::from_overrides(overrides).expect("scheduler settings")
    }
}
