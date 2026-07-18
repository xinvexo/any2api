use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use any2api_domain::{CredentialId, ProviderCredentialConfiguration};
use tokio::sync::watch;

use crate::{
    credential_runtime::{CredentialRuntimeBindings, CredentialRuntimeHandle},
    scheduler_epoch::SchedulerEpoch,
};

#[derive(Debug)]
pub struct RuntimeRegistry {
    scheduler_epoch: Arc<SchedulerEpoch>,
    credentials: RwLock<HashMap<CredentialId, Arc<CredentialRuntimeHandle>>>,
}

impl RuntimeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scheduler_epoch: SchedulerEpoch::new(),
            credentials: RwLock::new(HashMap::new()),
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
    ) -> CredentialRuntimeBindings {
        let mut handles = self
            .credentials
            .write()
            .expect("runtime credential registry lock poisoned");
        let mut active_ids = HashSet::with_capacity(configuration.credentials().len());
        let mut bindings = Vec::with_capacity(configuration.credentials().len());

        for credential in configuration.credentials() {
            active_ids.insert(credential.id());
            let handle = handles
                .entry(credential.id())
                .or_insert_with(|| {
                    CredentialRuntimeHandle::new(credential, Arc::clone(&self.scheduler_epoch))
                })
                .clone();
            bindings.push(handle.reconcile(credential));
        }

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
