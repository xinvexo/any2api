use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

use any2api_domain::{RequestsPerMinute, RoutingCredentialId};
use arc_swap::ArcSwap;
use tokio::time::Instant;

use super::{
    binding::CredentialRuntimeBinding,
    generation::{CredentialGenerationDefinition, CredentialGenerationRuntime},
    metrics::{CredentialBalancingCounters, CredentialBalancingMetrics, CredentialFilterKind},
    rate_window::{CredentialRateSnapshot, CredentialRateWindow, RateLimited},
};
use crate::routing::SchedulerEpoch;

#[cfg(test)]
use crate::credential::{CredentialAuthMaterial, CredentialAuthentication};
#[cfg(test)]
use any2api_domain::ProviderCredential;

#[derive(Debug)]
struct MutableRuntimeState {
    rate_window: CredentialRateWindow,
    fixed_waiters: u32,
}

pub(crate) struct CredentialRuntimeHandle {
    id: RoutingCredentialId,
    in_flight: AtomicU32,
    mutable: Mutex<MutableRuntimeState>,
    current_generation: ArcSwap<CredentialGenerationRuntime>,
    balancing: CredentialBalancingMetrics,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl CredentialRuntimeHandle {
    #[cfg(test)]
    pub(crate) fn new_for_provider_test(
        credential: &ProviderCredential,
        auth_material: CredentialAuthMaterial,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> CredentialRuntimeBinding {
        assert!(auth_material.matches(credential));
        let handle = Self::new(
            credential.id().into(),
            CredentialGenerationDefinition::new(
                credential.credential_generation(),
                credential.secret_version(),
                CredentialAuthentication::provider_api_key(auth_material.into_provider_secret()),
            ),
            scheduler_epoch,
        );
        handle.current_binding(credential.requests_per_minute())
    }

    pub(crate) fn new(
        id: RoutingCredentialId,
        generation: CredentialGenerationDefinition,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            in_flight: AtomicU32::new(0),
            mutable: Mutex::new(MutableRuntimeState {
                rate_window: CredentialRateWindow::new(),
                fixed_waiters: 0,
            }),
            current_generation: ArcSwap::from_pointee(CredentialGenerationRuntime::new(
                generation,
                Arc::clone(&scheduler_epoch),
            )),
            balancing: CredentialBalancingMetrics::default(),
            scheduler_epoch,
        })
    }

    pub(crate) fn reconcile(
        self: &Arc<Self>,
        requests_per_minute: Option<RequestsPerMinute>,
        generation: CredentialGenerationDefinition,
    ) -> CredentialRuntimeBinding {
        let current = self.current_generation.load_full();
        let generation = if current.matches(&generation) {
            current
        } else {
            let next = Arc::new(CredentialGenerationRuntime::new(
                generation,
                Arc::clone(&self.scheduler_epoch),
            ));
            self.current_generation.store(Arc::clone(&next));
            next
        };

        CredentialRuntimeBinding {
            handle: Arc::clone(self),
            generation,
            requests_per_minute,
        }
    }

    pub(crate) fn current_binding(
        self: &Arc<Self>,
        requests_per_minute: Option<RequestsPerMinute>,
    ) -> CredentialRuntimeBinding {
        CredentialRuntimeBinding {
            handle: Arc::clone(self),
            generation: self.current_generation.load_full(),
            requests_per_minute,
        }
    }

    pub(crate) const fn id(&self) -> RoutingCredentialId {
        self.id
    }

    pub(crate) fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::Acquire)
    }

    pub(crate) fn rate_snapshot(
        &self,
        requests_per_minute: Option<RequestsPerMinute>,
        now: Instant,
    ) -> CredentialRateSnapshot {
        self.mutable
            .lock()
            .expect("credential runtime lock poisoned")
            .rate_window
            .snapshot(now, requests_per_minute)
    }

    pub(crate) fn fixed_waiter_count(&self) -> u32 {
        self.mutable
            .lock()
            .expect("credential runtime lock poisoned")
            .fixed_waiters
    }

    pub(crate) fn balancing_counters(&self) -> CredentialBalancingCounters {
        self.balancing.snapshot()
    }

    pub(crate) fn record_selection(&self) {
        self.balancing.record_selection();
    }

    pub(crate) fn record_filter(&self, kind: CredentialFilterKind) {
        self.balancing.record_filter(kind);
    }

    pub(crate) fn try_reserve_normal(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
        requests_per_minute: Option<RequestsPerMinute>,
        now: Instant,
    ) -> Result<super::binding::RoutingPermit, RateLimited> {
        self.try_reserve(generation, requests_per_minute, now, false)
    }

    pub(crate) fn try_reserve_fixed(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
        requests_per_minute: Option<RequestsPerMinute>,
        now: Instant,
    ) -> Result<super::binding::RoutingPermit, RateLimited> {
        self.try_reserve(generation, requests_per_minute, now, true)
    }

    fn try_reserve(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
        requests_per_minute: Option<RequestsPerMinute>,
        now: Instant,
        fixed: bool,
    ) -> Result<super::binding::RoutingPermit, RateLimited> {
        {
            let mut state = self
                .mutable
                .lock()
                .expect("credential runtime lock poisoned");
            let fixed_waiters = state.fixed_waiters;
            state
                .rate_window
                .try_reserve(now, requests_per_minute, fixed_waiters, fixed)?;
        }
        self.in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .expect("credential in-flight counter overflowed u32");
        Ok(super::binding::RoutingPermit {
            handle: Arc::clone(self),
            generation,
        })
    }

    pub(crate) fn release_in_flight(&self) {
        self.in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(1)
            })
            .expect("routing permit released without an in-flight request");
    }

    pub(crate) fn register_fixed_waiter(self: &Arc<Self>) -> FixedCredentialWaiter {
        let mut state = self
            .mutable
            .lock()
            .expect("credential runtime lock poisoned");
        state.fixed_waiters = state
            .fixed_waiters
            .checked_add(1)
            .expect("fixed waiter counter overflowed u32");
        drop(state);
        self.scheduler_epoch.advance();
        FixedCredentialWaiter {
            handle: Arc::clone(self),
        }
    }

    fn release_fixed_waiter(&self) {
        let mut state = self
            .mutable
            .lock()
            .expect("credential runtime lock poisoned");
        state.fixed_waiters = state
            .fixed_waiters
            .checked_sub(1)
            .expect("fixed waiter released without registration");
        drop(state);
        self.scheduler_epoch.advance();
    }
}

impl fmt::Debug for CredentialRuntimeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialRuntimeHandle")
            .field("id", &self.id)
            .field("in_flight", &self.in_flight())
            .field("fixed_waiters", &self.fixed_waiter_count())
            .field("generation", &self.current_generation.load())
            .finish()
    }
}

pub(crate) struct FixedCredentialWaiter {
    handle: Arc<CredentialRuntimeHandle>,
}

impl Drop for FixedCredentialWaiter {
    fn drop(&mut self) {
        self.handle.release_fixed_waiter();
    }
}
