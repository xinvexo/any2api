use std::{collections::HashMap, fmt, sync::Arc};

use any2api_domain::CredentialId;
use any2api_provider::api::{CredentialHeaders, ProviderDriver, ProviderError};

use super::{
    capacity::CredentialCapacity,
    generation::CredentialGenerationRuntime,
    handle::{CredentialRuntimeHandle, FixedCredentialWaiter},
    metrics::{CredentialBalancingCounters, CredentialFilterKind},
};

#[derive(Clone, Debug)]
pub struct CredentialRuntimeBinding {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
    pub(crate) generation: Arc<CredentialGenerationRuntime>,
}

impl CredentialRuntimeBinding {
    #[must_use]
    pub fn credential_id(&self) -> CredentialId {
        self.handle.id()
    }

    #[must_use]
    pub fn capacity(&self) -> CredentialCapacity {
        self.handle.capacity()
    }

    pub(crate) fn normal_capacity(&self) -> CredentialCapacity {
        self.handle.normal_capacity()
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    #[must_use]
    pub fn is_retired(&self) -> bool {
        self.handle.is_retired()
    }

    pub(crate) fn auxiliary_in_flight(&self) -> u32 {
        self.handle.auxiliary_in_flight()
    }

    pub(crate) fn fixed_waiter_count(&self) -> u32 {
        self.handle.fixed_waiter_count()
    }

    pub(crate) fn balancing_counters(&self) -> CredentialBalancingCounters {
        self.handle.balancing_counters()
    }

    pub(crate) fn record_generation_selection(&self) {
        self.handle.record_generation_selection();
    }

    pub(crate) fn record_auxiliary_selection(&self) {
        self.handle.record_auxiliary_selection();
    }

    pub(crate) fn record_filter(&self, kind: CredentialFilterKind) {
        self.handle.record_filter(kind);
    }

    pub(crate) fn reserve_auxiliary(
        &self,
    ) -> (
        Arc<CredentialRuntimeHandle>,
        Arc<CredentialGenerationRuntime>,
    ) {
        self.handle.reserve_auxiliary();
        (Arc::clone(&self.handle), Arc::clone(&self.generation))
    }

    #[must_use]
    pub fn try_acquire(&self) -> Option<ConcurrencyPermit> {
        self.handle.try_acquire_normal(Arc::clone(&self.generation))
    }

    pub(crate) fn try_acquire_fixed(&self) -> Option<ConcurrencyPermit> {
        self.handle
            .try_acquire_unreserved(Arc::clone(&self.generation))
    }

    pub(crate) fn register_fixed_waiter(&self) -> FixedCredentialWaiter {
        self.handle.register_fixed_waiter()
    }
}

pub struct ConcurrencyPermit {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
    pub(crate) generation: Arc<CredentialGenerationRuntime>,
}

impl ConcurrencyPermit {
    #[must_use]
    pub fn credential_id(&self) -> CredentialId {
        self.handle.id()
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    pub fn provider_credential_headers(
        &self,
        driver: &dyn ProviderDriver,
    ) -> Result<CredentialHeaders, ProviderError> {
        driver.credential_headers(
            self.generation.credential_kind(),
            self.generation.provider_secret(),
        )
    }
}

impl fmt::Debug for ConcurrencyPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConcurrencyPermit")
            .field("credential_id", &self.handle.id())
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Drop for ConcurrencyPermit {
    fn drop(&mut self) {
        self.handle.release();
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CredentialRuntimeBindings {
    ordered: Vec<CredentialRuntimeBinding>,
    by_id: HashMap<CredentialId, usize>,
}

impl CredentialRuntimeBindings {
    pub(crate) fn new(ordered: Vec<CredentialRuntimeBinding>) -> Self {
        let by_id = ordered
            .iter()
            .enumerate()
            .map(|(index, binding)| (binding.credential_id(), index))
            .collect();
        Self { ordered, by_id }
    }

    pub(crate) fn get(&self, id: CredentialId) -> Option<&CredentialRuntimeBinding> {
        self.by_id.get(&id).map(|index| &self.ordered[*index])
    }

    pub(crate) fn as_slice(&self) -> &[CredentialRuntimeBinding] {
        &self.ordered
    }
}
