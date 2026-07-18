use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use any2api_domain::{CredentialId, MaxConcurrency, ProviderCredential};
use any2api_provider::api::{CredentialHeaders, ProviderDriver, ProviderError, ProviderSecret};
use arc_swap::ArcSwap;

use crate::{credential_auth::CredentialAuthMaterial, scheduler_epoch::SchedulerEpoch};

const IN_FLIGHT_MASK: u64 = u32::MAX as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CredentialCapacity {
    in_flight: u32,
    max_concurrency: u32,
}

impl CredentialCapacity {
    #[must_use]
    pub const fn in_flight(self) -> u32 {
        self.in_flight
    }

    #[must_use]
    pub const fn max_concurrency(self) -> u32 {
        self.max_concurrency
    }

    #[must_use]
    pub const fn is_full(self) -> bool {
        self.in_flight >= self.max_concurrency
    }
}

pub struct CredentialGenerationRuntime {
    credential_generation: u64,
    secret_version: u64,
    provider_secret: Arc<ProviderSecret>,
}

impl CredentialGenerationRuntime {
    fn new(credential: &ProviderCredential, auth_material: CredentialAuthMaterial) -> Self {
        assert!(
            auth_material.matches(credential),
            "Credential auth material does not match generation"
        );
        Self {
            credential_generation: credential.credential_generation(),
            secret_version: credential.secret_version(),
            provider_secret: auth_material.into_provider_secret(),
        }
    }

    #[must_use]
    pub const fn credential_generation(&self) -> u64 {
        self.credential_generation
    }

    #[must_use]
    pub const fn secret_version(&self) -> u64 {
        self.secret_version
    }

    pub(crate) fn provider_secret(&self) -> &ProviderSecret {
        self.provider_secret.as_ref()
    }

    fn matches(&self, credential: &ProviderCredential) -> bool {
        self.credential_generation == credential.credential_generation()
            && self.secret_version == credential.secret_version()
    }
}

impl fmt::Debug for CredentialGenerationRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialGenerationRuntime")
            .field("credential_generation", &self.credential_generation)
            .field("secret_version", &self.secret_version)
            .field("provider_secret", &"[REDACTED]")
            .finish()
    }
}

pub(crate) struct CredentialRuntimeHandle {
    id: CredentialId,
    capacity: AtomicU64,
    current_generation: ArcSwap<CredentialGenerationRuntime>,
    retired: AtomicBool,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl CredentialRuntimeHandle {
    pub(crate) fn new(
        credential: &ProviderCredential,
        auth_material: CredentialAuthMaterial,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id: credential.id(),
            capacity: AtomicU64::new(pack_capacity(credential.max_concurrency().get(), 0)),
            current_generation: ArcSwap::from_pointee(CredentialGenerationRuntime::new(
                credential,
                auth_material,
            )),
            retired: AtomicBool::new(false),
            scheduler_epoch,
        })
    }

    pub(crate) fn reconcile(
        self: &Arc<Self>,
        credential: &ProviderCredential,
        auth_material: CredentialAuthMaterial,
    ) -> CredentialRuntimeBinding {
        assert_eq!(self.id, credential.id(), "credential runtime id changed");
        self.update_max_concurrency(credential.max_concurrency());
        self.retired.store(false, Ordering::Release);

        let current = self.current_generation.load_full();
        let generation = if current.matches(credential) {
            current
        } else {
            let next = Arc::new(CredentialGenerationRuntime::new(credential, auth_material));
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

    fn capacity(&self) -> CredentialCapacity {
        unpack_capacity(self.capacity.load(Ordering::Acquire))
    }

    fn update_max_concurrency(&self, max_concurrency: MaxConcurrency) {
        let max_concurrency = max_concurrency.get();
        let mut current = self.capacity.load(Ordering::Acquire);
        loop {
            let capacity = unpack_capacity(current);
            let next = pack_capacity(max_concurrency, capacity.in_flight);
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

    fn try_acquire(
        self: &Arc<Self>,
        generation: Arc<CredentialGenerationRuntime>,
    ) -> Option<ConcurrencyPermit> {
        let mut current = self.capacity.load(Ordering::Acquire);
        loop {
            let capacity = unpack_capacity(current);
            if capacity.is_full() {
                return None;
            }
            let next = pack_capacity(capacity.max_concurrency, capacity.in_flight + 1);
            match self.capacity.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(ConcurrencyPermit {
                        handle: Arc::clone(self),
                        generation,
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn release(&self) {
        self.capacity
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                let capacity = unpack_capacity(current);
                (capacity.in_flight > 0)
                    .then(|| pack_capacity(capacity.max_concurrency, capacity.in_flight - 1))
            })
            .expect("concurrency permit released without an active slot");
        self.scheduler_epoch.advance();
    }
}

impl fmt::Debug for CredentialRuntimeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialRuntimeHandle")
            .field("id", &self.id)
            .field("capacity", &self.capacity())
            .field("generation", &self.current_generation.load())
            .field("retired", &self.retired.load(Ordering::Acquire))
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct CredentialRuntimeBinding {
    handle: Arc<CredentialRuntimeHandle>,
    generation: Arc<CredentialGenerationRuntime>,
}

impl CredentialRuntimeBinding {
    #[must_use]
    pub fn credential_id(&self) -> CredentialId {
        self.handle.id
    }

    #[must_use]
    pub fn capacity(&self) -> CredentialCapacity {
        self.handle.capacity()
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    #[must_use]
    pub fn is_retired(&self) -> bool {
        self.handle.retired.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn try_acquire(&self) -> Option<ConcurrencyPermit> {
        self.handle.try_acquire(Arc::clone(&self.generation))
    }
}

pub struct ConcurrencyPermit {
    handle: Arc<CredentialRuntimeHandle>,
    generation: Arc<CredentialGenerationRuntime>,
}

impl ConcurrencyPermit {
    #[must_use]
    pub fn credential_id(&self) -> CredentialId {
        self.handle.id
    }

    #[must_use]
    pub fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    pub fn provider_credential_headers(
        &self,
        driver: &dyn ProviderDriver,
    ) -> Result<CredentialHeaders, ProviderError> {
        driver.credential_headers(self.generation.provider_secret())
    }
}

impl fmt::Debug for ConcurrencyPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConcurrencyPermit")
            .field("credential_id", &self.handle.id)
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

const fn pack_capacity(max_concurrency: u32, in_flight: u32) -> u64 {
    ((max_concurrency as u64) << 32) | in_flight as u64
}

const fn unpack_capacity(state: u64) -> CredentialCapacity {
    CredentialCapacity {
        in_flight: (state & IN_FLIGHT_MASK) as u32,
        max_concurrency: (state >> 32) as u32,
    }
}
