use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
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
    token_window::{CredentialTokenUsageRecorder, CredentialTokenUsageWindow},
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
    retired: AtomicBool,
    balancing: CredentialBalancingMetrics,
    token_usage: Arc<CredentialTokenUsageWindow>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl CredentialRuntimeHandle {
    #[cfg(test)]
    pub(crate) fn new_for_provider_test(
        credential: &ProviderCredential,
        auth_material: CredentialAuthMaterial,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Arc<Self> {
        assert!(auth_material.matches(credential));
        Self::new(
            credential.id().into(),
            credential.requests_per_minute(),
            CredentialGenerationDefinition::new(
                credential.credential_generation(),
                credential.secret_version(),
                CredentialAuthentication::provider_api_key(auth_material.into_provider_secret()),
            ),
            scheduler_epoch,
        )
    }

    pub(crate) fn new(
        id: RoutingCredentialId,
        requests_per_minute: Option<RequestsPerMinute>,
        generation: CredentialGenerationDefinition,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            in_flight: AtomicU32::new(0),
            mutable: Mutex::new(MutableRuntimeState {
                rate_window: CredentialRateWindow::new(requests_per_minute),
                fixed_waiters: 0,
            }),
            current_generation: ArcSwap::from_pointee(CredentialGenerationRuntime::new(
                generation,
                Arc::clone(&scheduler_epoch),
            )),
            retired: AtomicBool::new(false),
            balancing: CredentialBalancingMetrics::default(),
            token_usage: Arc::new(CredentialTokenUsageWindow::default()),
            scheduler_epoch,
        })
    }

    pub(crate) fn reconcile(
        self: &Arc<Self>,
        id: RoutingCredentialId,
        requests_per_minute: Option<RequestsPerMinute>,
        generation: CredentialGenerationDefinition,
    ) -> CredentialRuntimeBinding {
        assert_eq!(self.id, id, "credential runtime id changed");
        self.mutable
            .lock()
            .expect("credential runtime lock poisoned")
            .rate_window
            .reconcile(requests_per_minute, Instant::now());
        self.retired.store(false, Ordering::Release);

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
        }
    }

    pub(crate) fn current_binding(self: &Arc<Self>) -> CredentialRuntimeBinding {
        CredentialRuntimeBinding {
            handle: Arc::clone(self),
            generation: self.current_generation.load_full(),
        }
    }

    pub(crate) fn retire(&self) {
        self.retired.store(true, Ordering::Release);
    }

    pub(crate) const fn id(&self) -> RoutingCredentialId {
        self.id
    }

    pub(crate) fn is_retired(&self) -> bool {
        self.retired.load(Ordering::Acquire)
    }

    pub(crate) fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::Acquire)
    }

    pub(crate) fn rate_snapshot(&self, now: Instant) -> CredentialRateSnapshot {
        self.mutable
            .lock()
            .expect("credential runtime lock poisoned")
            .rate_window
            .snapshot(now)
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

    pub(crate) fn token_usage_recorder(&self) -> CredentialTokenUsageRecorder {
        self.token_usage.recorder()
    }

    pub(crate) fn token_usage_snapshot(&self, window_seconds: u64) -> u64 {
        self.token_usage.snapshot(window_seconds)
    }

    pub(crate) fn try_reserve_normal(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
        now: Instant,
    ) -> Result<super::binding::RoutingPermit, RateLimited> {
        self.try_reserve(generation, now, false)
    }

    pub(crate) fn try_reserve_fixed(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
        now: Instant,
    ) -> Result<super::binding::RoutingPermit, RateLimited> {
        self.try_reserve(generation, now, true)
    }

    fn try_reserve(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
        now: Instant,
        fixed: bool,
    ) -> Result<super::binding::RoutingPermit, RateLimited> {
        {
            let mut state = self
                .mutable
                .lock()
                .expect("credential runtime lock poisoned");
            let fixed_waiters = state.fixed_waiters;
            state.rate_window.try_reserve(now, fixed_waiters, fixed)?;
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
            .field("rate", &self.rate_snapshot(Instant::now()))
            .field("fixed_waiters", &self.fixed_waiter_count())
            .field("generation", &self.current_generation.load())
            .field("retired", &self.retired.load(Ordering::Acquire))
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
