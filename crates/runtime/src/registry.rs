use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use any2api_domain::{CredentialId, ModelRouteConfiguration, ProviderCredentialConfiguration};
use tokio::sync::watch;

use crate::{
    credential_auth::CredentialAuthMaterials,
    credential_runtime::{CredentialRuntimeBindings, CredentialRuntimeHandle},
    route_tier_cursor::{RouteTierCursorBindings, RouteTierCursorRegistry},
    scheduler_epoch::SchedulerEpoch,
};

#[derive(Debug)]
pub struct RuntimeRegistry {
    scheduler_epoch: Arc<SchedulerEpoch>,
    credentials: RwLock<HashMap<CredentialId, Arc<CredentialRuntimeHandle>>>,
    route_tier_cursors: RouteTierCursorRegistry,
}

impl RuntimeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scheduler_epoch: SchedulerEpoch::new(),
            credentials: RwLock::new(HashMap::new()),
            route_tier_cursors: RouteTierCursorRegistry::default(),
        }
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

        CredentialRuntimeBindings::new(bindings)
    }

    pub(crate) fn reconcile_route_tier_cursors(
        &self,
        configuration: &ModelRouteConfiguration,
    ) -> RouteTierCursorBindings {
        self.route_tier_cursors.reconcile(configuration)
    }
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::new()
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
