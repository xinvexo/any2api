use std::{fmt, sync::Arc};

use any2api_domain::RoutingCredentialId;
use any2api_provider::api::{CredentialHeaders, ProviderDriver, ProviderError};
use http::HeaderMap;
use tokio::time::Instant;

use super::{
    generation::CredentialGenerationRuntime,
    handle::{CredentialRuntimeHandle, FixedCredentialWaiter},
    metrics::{CredentialBalancingCounters, CredentialFilterKind},
    rate_window::{CredentialRateSnapshot, RateLimited},
};

#[derive(Clone, Debug)]
pub struct CredentialRuntimeBinding {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
    pub(crate) generation: Arc<CredentialGenerationRuntime>,
}

impl CredentialRuntimeBinding {
    #[must_use]
    pub fn credential_id(&self) -> RoutingCredentialId {
        self.handle.id()
    }

    #[must_use]
    pub fn in_flight(&self) -> u32 {
        self.handle.in_flight()
    }

    #[must_use]
    pub fn rate_snapshot(&self) -> CredentialRateSnapshot {
        self.handle.rate_snapshot(Instant::now())
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    #[must_use]
    pub fn is_retired(&self) -> bool {
        self.handle.is_retired()
    }

    pub(crate) fn fixed_waiter_count(&self) -> u32 {
        self.handle.fixed_waiter_count()
    }

    pub(crate) fn balancing_counters(&self) -> CredentialBalancingCounters {
        self.handle.balancing_counters()
    }

    pub(crate) fn record_selection(&self) {
        self.handle.record_selection();
    }

    pub(crate) fn record_filter(&self, kind: CredentialFilterKind) {
        self.handle.record_filter(kind);
    }

    pub(crate) fn try_reserve(&self) -> Result<RoutingPermit, RateLimited> {
        self.handle
            .try_reserve_normal(Arc::clone(&self.generation), Instant::now())
    }

    pub(crate) fn try_reserve_fixed(&self) -> Result<RoutingPermit, RateLimited> {
        self.handle
            .try_reserve_fixed(Arc::clone(&self.generation), Instant::now())
    }

    pub(crate) fn register_fixed_waiter(&self) -> FixedCredentialWaiter {
        self.handle.register_fixed_waiter()
    }
}

pub struct RoutingPermit {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
    pub(crate) generation: Arc<CredentialGenerationRuntime>,
}

impl RoutingPermit {
    #[must_use]
    pub fn credential_id(&self) -> RoutingCredentialId {
        self.handle.id()
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    pub fn credential_headers(
        &self,
        driver: &dyn ProviderDriver,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        self.generation.credential_headers(driver, forwarded)
    }
}

impl fmt::Debug for RoutingPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutingPermit")
            .field("credential_id", &self.handle.id())
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Drop for RoutingPermit {
    fn drop(&mut self) {
        self.handle.release_in_flight();
    }
}

#[derive(Clone, Debug, Default)]
#[cfg(test)]
pub(crate) struct CredentialRuntimeBindings {
    ordered: Vec<CredentialRuntimeBinding>,
}

#[cfg(test)]
impl CredentialRuntimeBindings {
    pub(crate) fn new(ordered: Vec<CredentialRuntimeBinding>) -> Self {
        Self { ordered }
    }

    pub(crate) fn as_slice(&self) -> &[CredentialRuntimeBinding] {
        &self.ordered
    }
}
