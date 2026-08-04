use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_protocol::api::{MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolContinuationState};

use super::{
    continuation_lease::ContinuationLease,
    registry::{
        AffinityError, AffinityRegistry, BindingSource, BindingState, ContinuationLifecycle,
        TimedBinding,
    },
    target::AffinityTarget,
};

pub(super) const MAX_BRIDGE_CONTINUATION_TOTAL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct ResolvedContinuation {
    target: AffinityTarget,
    state: Option<ProtocolContinuationState>,
}

impl ResolvedContinuation {
    pub(crate) fn into_parts(self) -> (AffinityTarget, Option<ProtocolContinuationState>) {
        (self.target, self.state)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ContinuationLookup {
    Missing,
    Pending,
    Ready(ResolvedContinuation),
}

impl AffinityRegistry {
    pub(crate) fn resolve_continuation(
        &self,
        raw: &str,
        ttl: Duration,
        target_is_available: impl FnOnce(&AffinityTarget) -> bool,
    ) -> ContinuationLookup {
        let key = self.hasher.continuation(raw);
        let now = Instant::now();
        let mut shard = self.bindings.lock(key);
        let Some(BindingState::Bound { binding }) = shard.get_mut(&key) else {
            return ContinuationLookup::Missing;
        };
        match &binding.source {
            BindingSource::Continuation(ContinuationLifecycle::Pending { .. }) => {
                ContinuationLookup::Pending
            }
            BindingSource::Continuation(ContinuationLifecycle::Ready {
                state: continuation,
            }) => {
                if now.saturating_duration_since(binding.last_seen_at) >= ttl {
                    shard.remove(&key);
                    return ContinuationLookup::Missing;
                }
                if !target_is_available(&binding.target) {
                    return ContinuationLookup::Missing;
                }
                binding.last_seen_at = now;
                ContinuationLookup::Ready(ResolvedContinuation {
                    target: binding.target.clone(),
                    state: continuation.clone(),
                })
            }
            BindingSource::Session => ContinuationLookup::Missing,
        }
    }

    pub(crate) fn bind_ready_continuation(
        &self,
        raw: &str,
        target: AffinityTarget,
        continuation: Option<ProtocolContinuationState>,
        ttl: Duration,
    ) -> Result<(), AffinityError> {
        self.bind_ready_continuation_with_deadline(raw, target, continuation, ttl, None)
    }

    pub(crate) fn bind_ready_continuation_before(
        &self,
        raw: &str,
        target: AffinityTarget,
        continuation: Option<ProtocolContinuationState>,
        ttl: Duration,
        deadline: Instant,
    ) -> Result<(), AffinityError> {
        self.bind_ready_continuation_with_deadline(raw, target, continuation, ttl, Some(deadline))
    }

    pub(crate) fn begin_pending_continuation(
        self: &Arc<Self>,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
    ) -> Result<ContinuationLease, AffinityError> {
        self.begin_pending_continuation_with_deadline(raw, target, ttl, None)
    }

    pub(crate) fn begin_pending_continuation_before(
        self: &Arc<Self>,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
        deadline: Instant,
    ) -> Result<ContinuationLease, AffinityError> {
        self.begin_pending_continuation_with_deadline(raw, target, ttl, Some(deadline))
    }

    fn bind_ready_continuation_with_deadline(
        &self,
        raw: &str,
        target: AffinityTarget,
        continuation: Option<ProtocolContinuationState>,
        ttl: Duration,
        deadline: Option<Instant>,
    ) -> Result<(), AffinityError> {
        check_deadline(deadline)?;
        checked_state_bytes(continuation.as_ref())?;
        let key = self.hasher.continuation(raw);
        let now = Instant::now();
        let mut reclaimed = false;
        loop {
            let mut shard = self.bindings.lock(key);
            check_deadline(deadline)?;
            shard.remove_if_expired(&key, now, ttl);
            if let Some(existing) = shard.get_mut(&key) {
                let BindingState::Bound { binding } = existing else {
                    return Err(AffinityError::IdentityConflict);
                };
                let idempotent_stateless = binding.target == target
                    && continuation.is_none()
                    && matches!(
                        binding.source,
                        BindingSource::Continuation(ContinuationLifecycle::Ready { state: None })
                    );
                if !idempotent_stateless {
                    return Err(AffinityError::IdentityConflict);
                }
                check_deadline(deadline)?;
                binding.last_seen_at = Instant::now();
                return Ok(());
            }
            shard.reclaim_expired_sample(now, ttl);
            check_deadline(deadline)?;
            let bound = BindingState::Bound {
                binding: TimedBinding {
                    target: target.clone(),
                    last_seen_at: Instant::now(),
                    source: BindingSource::Continuation(ContinuationLifecycle::Ready {
                        // Continuation state is Arc-backed, so the retry
                        // clone is cheap.
                        state: continuation.clone(),
                    }),
                },
            };
            match shard.try_insert(key, bound) {
                Ok(()) => return Ok(()),
                Err(rejection) if reclaimed => return Err(rejection.into()),
                Err(rejection) => {
                    drop(shard);
                    if !self.bindings.reclaim_for_capacity(now, ttl) {
                        return Err(rejection.into());
                    }
                    reclaimed = true;
                }
            }
        }
    }

    fn begin_pending_continuation_with_deadline(
        self: &Arc<Self>,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
        deadline: Option<Instant>,
    ) -> Result<ContinuationLease, AffinityError> {
        check_deadline(deadline)?;
        let key = self.hasher.continuation(raw);
        let now = Instant::now();
        let mut reclaimed = false;
        loop {
            let mut shard = self.bindings.lock(key);
            check_deadline(deadline)?;
            shard.remove_if_expired(&key, now, ttl);
            if shard.contains_key(&key) {
                return Err(AffinityError::IdentityConflict);
            }
            shard.reclaim_expired_sample(now, ttl);
            check_deadline(deadline)?;
            let version = self.bindings.next_version();
            let bound = BindingState::Bound {
                binding: TimedBinding {
                    target: target.clone(),
                    last_seen_at: Instant::now(),
                    source: BindingSource::Continuation(ContinuationLifecycle::Pending { version }),
                },
            };
            match shard.try_insert(key, bound) {
                Ok(()) => {
                    return Ok(ContinuationLease {
                        registry: Arc::clone(self),
                        key,
                        version,
                        target,
                        active: true,
                    });
                }
                Err(rejection) if reclaimed => return Err(rejection.into()),
                Err(rejection) => {
                    drop(shard);
                    if !self.bindings.reclaim_for_capacity(now, ttl) {
                        return Err(rejection.into());
                    }
                    reclaimed = true;
                }
            }
        }
    }
}

pub(super) fn checked_state_bytes(
    continuation: Option<&ProtocolContinuationState>,
) -> Result<usize, AffinityError> {
    let bytes = continuation.map_or(0, ProtocolContinuationState::serialized_bytes);
    (bytes <= MAX_BRIDGE_CONTINUATION_STATE_BYTES)
        .then_some(bytes)
        .ok_or(AffinityError::ContinuationTooLarge)
}

pub(super) fn check_deadline(deadline: Option<Instant>) -> Result<(), AffinityError> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(AffinityError::DeadlineExceeded);
    }
    Ok(())
}

#[cfg(test)]
pub(super) const TEST_TOTAL_BYTES: usize = MAX_BRIDGE_CONTINUATION_TOTAL_BYTES;
