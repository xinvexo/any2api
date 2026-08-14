use std::{fmt, sync::Arc};

use any2api_domain::{ProviderBaseUrl, RequestsPerMinute, RoutingCredentialId};
use any2api_provider::api::{CredentialHeaders, ProviderDriver, ProviderError};
use any2api_transport::api::{TransportIsolationKey, TransportTrafficClass};
use http::HeaderMap;
use tokio::time::Instant;

use super::{
    generation::CredentialGenerationRuntime,
    handle::{CredentialRuntimeHandle, FixedCredentialWaiter},
    metrics::{CredentialBalancingCounters, CredentialFilterKind},
    rate_window::{CredentialRateSnapshot, RateLimited, RateReservation},
};

#[derive(Clone, Debug)]
pub struct CredentialRuntimeBinding {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
    pub(crate) generation: Arc<CredentialGenerationRuntime>,
    pub(crate) requests_per_minute: Option<RequestsPerMinute>,
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
        self.handle
            .rate_snapshot(self.requests_per_minute, Instant::now())
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    pub(crate) fn transport_isolation(
        &self,
        traffic_class: TransportTrafficClass,
    ) -> TransportIsolationKey {
        transport_isolation(
            self.credential_id(),
            self.generation.as_ref(),
            traffic_class,
        )
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
            .try_reserve_normal(Arc::clone(&self.generation), self.requests_per_minute)
    }

    pub(crate) fn try_reserve_fixed(&self) -> Result<RoutingPermit, RateLimited> {
        self.handle
            .try_reserve_fixed(Arc::clone(&self.generation), self.requests_per_minute)
    }

    pub(crate) fn register_fixed_waiter(&self) -> FixedCredentialWaiter {
        self.handle.register_fixed_waiter()
    }

    pub(crate) fn subscribe_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.handle.subscribe_changes()
    }
}

pub struct RoutingPermit {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
    pub(crate) generation: Arc<CredentialGenerationRuntime>,
    pub(super) rate_reservation: Option<RateReservation>,
    pub(super) in_flight_released: bool,
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
        base_url: &ProviderBaseUrl,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        self.generation
            .credential_headers(driver, base_url, forwarded)
    }

    pub(crate) fn transport_isolation(
        &self,
        traffic_class: TransportTrafficClass,
    ) -> TransportIsolationKey {
        transport_isolation(
            self.credential_id(),
            self.generation.as_ref(),
            traffic_class,
        )
    }

    pub(crate) fn rollback_before_attempt(mut self) {
        self.in_flight_released = true;
        self.handle
            .rollback_before_attempt(self.rate_reservation.take());
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
        if !self.in_flight_released {
            self.handle.release_in_flight();
        }
    }
}

fn transport_isolation(
    credential_id: RoutingCredentialId,
    generation: &CredentialGenerationRuntime,
    traffic_class: TransportTrafficClass,
) -> TransportIsolationKey {
    TransportIsolationKey::routing_credential(
        credential_id,
        generation.routing_generation(),
        generation.authentication_version(),
        traffic_class,
    )
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
