use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};

use any2api_domain::{MaxConcurrency, RoutingCredentialId};
use arc_swap::ArcSwap;

use super::{
    binding::CredentialRuntimeBinding,
    capacity::{CredentialCapacity, pack, unpack},
    generation::{CredentialGenerationDefinition, CredentialGenerationRuntime},
    metrics::{CredentialBalancingCounters, CredentialBalancingMetrics, CredentialFilterKind},
};
use crate::scheduler_epoch::SchedulerEpoch;

#[cfg(test)]
use crate::{
    credential_auth::CredentialAuthMaterial, credential_runtime::CredentialAuthentication,
};
#[cfg(test)]
use any2api_domain::ProviderCredential;

pub(crate) struct CredentialRuntimeHandle {
    id: RoutingCredentialId,
    capacity: AtomicU64,
    fixed_waiters: AtomicU32,
    auxiliary_in_flight: AtomicU32,
    current_generation: ArcSwap<CredentialGenerationRuntime>,
    retired: AtomicBool,
    balancing: CredentialBalancingMetrics,
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
            credential.max_concurrency(),
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
        max_concurrency: MaxConcurrency,
        generation: CredentialGenerationDefinition,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            capacity: AtomicU64::new(pack(max_concurrency.get(), 0)),
            fixed_waiters: AtomicU32::new(0),
            auxiliary_in_flight: AtomicU32::new(0),
            current_generation: ArcSwap::from_pointee(CredentialGenerationRuntime::new(
                generation,
                Arc::clone(&scheduler_epoch),
            )),
            retired: AtomicBool::new(false),
            balancing: CredentialBalancingMetrics::default(),
            scheduler_epoch,
        })
    }

    pub(crate) fn reconcile(
        self: &Arc<Self>,
        id: RoutingCredentialId,
        max_concurrency: MaxConcurrency,
        generation: CredentialGenerationDefinition,
    ) -> CredentialRuntimeBinding {
        assert_eq!(self.id, id, "credential runtime id changed");
        self.update_max_concurrency(max_concurrency);
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

    pub(crate) fn capacity(&self) -> CredentialCapacity {
        unpack(self.capacity.load(Ordering::Acquire))
    }

    pub(crate) fn normal_capacity(&self) -> CredentialCapacity {
        let capacity = self.capacity();
        if self.fixed_waiters.load(Ordering::Acquire) == 0 {
            capacity
        } else {
            CredentialCapacity::full(capacity.max_concurrency())
        }
    }

    pub(crate) const fn id(&self) -> RoutingCredentialId {
        self.id
    }

    pub(crate) fn is_retired(&self) -> bool {
        self.retired.load(Ordering::Acquire)
    }

    pub(crate) fn auxiliary_in_flight(&self) -> u32 {
        self.auxiliary_in_flight.load(Ordering::Acquire)
    }

    pub(crate) fn fixed_waiter_count(&self) -> u32 {
        self.fixed_waiters.load(Ordering::Acquire)
    }

    pub(crate) fn balancing_counters(&self) -> CredentialBalancingCounters {
        self.balancing.snapshot()
    }

    pub(crate) fn record_generation_selection(&self) {
        self.balancing.record_generation_selection();
    }

    pub(crate) fn record_auxiliary_selection(&self) {
        self.balancing.record_auxiliary_selection();
    }

    pub(crate) fn record_filter(&self, kind: CredentialFilterKind) {
        self.balancing.record_filter(kind);
    }

    pub(crate) fn reserve_auxiliary(&self) {
        self.auxiliary_in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .expect("auxiliary in-flight counter overflowed u32");
    }

    pub(crate) fn release_auxiliary(&self) {
        self.auxiliary_in_flight
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(1)
            })
            .expect("auxiliary permit released without an active slot");
    }

    fn update_max_concurrency(&self, max_concurrency: MaxConcurrency) {
        let max_concurrency = max_concurrency.get();
        let mut current = self.capacity.load(Ordering::Acquire);
        loop {
            let capacity = unpack(current);
            let next = pack(max_concurrency, capacity.in_flight());
            match self.capacity.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    pub(crate) fn try_acquire_normal(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
    ) -> Option<super::binding::ConcurrencyPermit> {
        if self.fixed_waiters.load(Ordering::Acquire) > 0 {
            return None;
        }
        let permit = self.try_acquire_unreserved(generation)?;
        if self.fixed_waiters.load(Ordering::Acquire) == 0 {
            return Some(permit);
        }
        drop(permit);
        None
    }

    pub(crate) fn try_acquire_unreserved(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
    ) -> Option<super::binding::ConcurrencyPermit> {
        let mut current = self.capacity.load(Ordering::Acquire);
        loop {
            let capacity = unpack(current);
            if capacity.is_full() {
                return None;
            }
            let next = pack(capacity.max_concurrency(), capacity.in_flight() + 1);
            match self.capacity.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(super::binding::ConcurrencyPermit {
                        handle: Arc::clone(self),
                        generation,
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }

    pub(crate) fn release(&self) {
        self.capacity
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                let capacity = unpack(current);
                (capacity.in_flight() > 0)
                    .then(|| pack(capacity.max_concurrency(), capacity.in_flight() - 1))
            })
            .expect("concurrency permit released without an active slot");
        self.scheduler_epoch.advance();
    }

    pub(crate) fn register_fixed_waiter(self: &Arc<Self>) -> FixedCredentialWaiter {
        self.fixed_waiters
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .expect("fixed waiter counter overflowed u32");
        self.scheduler_epoch.advance();
        FixedCredentialWaiter {
            handle: Arc::clone(self),
        }
    }

    pub(crate) fn release_fixed_waiter(&self) {
        self.fixed_waiters
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_sub(1)
            })
            .expect("fixed waiter released without registration");
        self.scheduler_epoch.advance();
    }
}

impl fmt::Debug for CredentialRuntimeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialRuntimeHandle")
            .field("id", &self.id)
            .field("capacity", &self.capacity())
            .field("fixed_waiters", &self.fixed_waiters.load(Ordering::Acquire))
            .field("auxiliary_in_flight", &self.auxiliary_in_flight())
            .field("generation", &self.current_generation.load())
            .field("retired", &self.retired.load(Ordering::Acquire))
            .finish()
    }
}

pub(crate) struct FixedCredentialWaiter {
    pub(crate) handle: Arc<CredentialRuntimeHandle>,
}

impl Drop for FixedCredentialWaiter {
    fn drop(&mut self) {
        self.handle.release_fixed_waiter();
    }
}
