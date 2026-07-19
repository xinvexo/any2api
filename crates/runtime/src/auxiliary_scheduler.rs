use std::{
    cmp::Ordering,
    fmt,
    sync::{Arc, Mutex},
};

use any2api_provider::api::{CredentialHeaders, ProviderDriver, ProviderError};
use thiserror::Error;

#[cfg(test)]
use any2api_domain::CredentialId;

use crate::{
    credential_runtime::{
        CredentialGenerationRuntime, CredentialRuntimeBinding, CredentialRuntimeHandle,
    },
    scheduler_epoch::SchedulerEpoch,
};

const DEFAULT_GLOBAL_AUXILIARY_CONCURRENCY: u32 = 32;
const DEFAULT_PER_CREDENTIAL_AUXILIARY_CONCURRENCY: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuxiliaryConcurrencyLimits {
    global: u32,
    per_credential: u32,
}

impl AuxiliaryConcurrencyLimits {
    pub const fn new(
        global: u32,
        per_credential: u32,
    ) -> Result<Self, AuxiliaryConcurrencyLimitsError> {
        if global == 0 {
            return Err(AuxiliaryConcurrencyLimitsError::ZeroGlobal);
        }
        if per_credential == 0 {
            return Err(AuxiliaryConcurrencyLimitsError::ZeroPerCredential);
        }
        Ok(Self {
            global,
            per_credential,
        })
    }

    #[must_use]
    pub const fn global(self) -> u32 {
        self.global
    }

    #[must_use]
    pub const fn per_credential(self) -> u32 {
        self.per_credential
    }
}

impl Default for AuxiliaryConcurrencyLimits {
    fn default() -> Self {
        Self {
            global: DEFAULT_GLOBAL_AUXILIARY_CONCURRENCY,
            per_credential: DEFAULT_PER_CREDENTIAL_AUXILIARY_CONCURRENCY,
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum AuxiliaryConcurrencyLimitsError {
    #[error("global auxiliary concurrency must be greater than zero")]
    ZeroGlobal,
    #[error("per-credential auxiliary concurrency must be greater than zero")]
    ZeroPerCredential,
}

#[derive(Debug)]
struct AuxiliaryState {
    limits: AuxiliaryConcurrencyLimits,
    global_in_flight: u32,
}

#[derive(Debug)]
pub(crate) struct AuxiliaryScheduler {
    state: Mutex<AuxiliaryState>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl AuxiliaryScheduler {
    pub(crate) fn new(
        limits: AuxiliaryConcurrencyLimits,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(AuxiliaryState {
                limits,
                global_in_flight: 0,
            }),
            scheduler_epoch,
        })
    }

    pub(crate) fn limits(&self) -> AuxiliaryConcurrencyLimits {
        self.state
            .lock()
            .expect("auxiliary scheduler lock poisoned")
            .limits
    }

    pub(crate) fn update_limits(&self, limits: AuxiliaryConcurrencyLimits) {
        let changed = {
            let mut state = self
                .state
                .lock()
                .expect("auxiliary scheduler lock poisoned");
            if state.limits == limits {
                false
            } else {
                state.limits = limits;
                true
            }
        };
        if changed {
            self.scheduler_epoch.advance();
        }
    }

    pub(crate) fn select_index_and_try_acquire(
        self: &Arc<Self>,
        candidates: &[CredentialRuntimeBinding],
        tie_breaker: u64,
    ) -> AuxiliarySelectAndAcquireResult {
        if candidates.is_empty() {
            return AuxiliarySelectAndAcquireResult::NoCandidates;
        }

        let mut state = self
            .state
            .lock()
            .expect("auxiliary scheduler lock poisoned");
        if state.global_in_flight >= state.limits.global {
            return AuxiliarySelectAndAcquireResult::AtCapacity;
        }
        let Some(index) = select_available(candidates, state.limits.per_credential, tie_breaker)
        else {
            return AuxiliarySelectAndAcquireResult::AtCapacity;
        };
        let (handle, generation) = candidates[index].reserve_auxiliary();
        state.global_in_flight += 1;
        drop(state);

        AuxiliarySelectAndAcquireResult::Acquired {
            index,
            permit: AuxiliaryPermit {
                scheduler: Arc::clone(self),
                handle,
                generation,
            },
        }
    }

    fn release(&self, handle: &CredentialRuntimeHandle) {
        {
            let mut state = self
                .state
                .lock()
                .expect("auxiliary scheduler lock poisoned");
            assert!(
                state.global_in_flight > 0,
                "auxiliary permit released without a global slot"
            );
            state.global_in_flight -= 1;
            handle.release_auxiliary();
        }
        self.scheduler_epoch.advance();
    }

    #[cfg(test)]
    pub(crate) fn global_in_flight(&self) -> u32 {
        self.state
            .lock()
            .expect("auxiliary scheduler lock poisoned")
            .global_in_flight
    }
}

#[derive(Debug)]
pub(crate) enum AuxiliarySelectAndAcquireResult {
    Acquired {
        index: usize,
        permit: AuxiliaryPermit,
    },
    AtCapacity,
    NoCandidates,
}

pub(crate) struct AuxiliaryPermit {
    scheduler: Arc<AuxiliaryScheduler>,
    handle: Arc<CredentialRuntimeHandle>,
    generation: Arc<CredentialGenerationRuntime>,
}

impl AuxiliaryPermit {
    #[cfg(test)]
    #[must_use]
    pub(crate) fn credential_id(&self) -> CredentialId {
        self.handle.id()
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn generation(&self) -> &Arc<CredentialGenerationRuntime> {
        &self.generation
    }

    pub(crate) fn provider_credential_headers(
        &self,
        driver: &dyn ProviderDriver,
    ) -> Result<CredentialHeaders, ProviderError> {
        driver.credential_headers(self.generation.provider_secret())
    }
}

impl fmt::Debug for AuxiliaryPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuxiliaryPermit")
            .field("credential_id", &self.handle.id())
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl Drop for AuxiliaryPermit {
    fn drop(&mut self) {
        self.scheduler.release(self.handle.as_ref());
    }
}

fn select_available(
    candidates: &[CredentialRuntimeBinding],
    per_credential_limit: u32,
    tie_breaker: u64,
) -> Option<usize> {
    let start = usize::try_from(tie_breaker % candidates.len() as u64)
        .expect("tie breaker is bounded by candidate count");
    let mut best: Option<(usize, u32)> = None;

    for (index, candidate) in candidates.iter().enumerate() {
        let in_flight = candidate.auxiliary_in_flight();
        if in_flight >= per_credential_limit {
            continue;
        }
        let replace = best.is_none_or(|(best_index, best_in_flight)| {
            in_flight.cmp(&best_in_flight).then_with(|| {
                cyclic_rank(index, start, candidates.len()).cmp(&cyclic_rank(
                    best_index,
                    start,
                    candidates.len(),
                ))
            }) == Ordering::Less
        });
        if replace {
            best = Some((index, in_flight));
        }
    }

    best.map(|(index, _)| index)
}

const fn cyclic_rank(index: usize, start: usize, length: usize) -> usize {
    (index + length - start) % length
}
