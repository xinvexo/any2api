use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_domain::{ModelRouteId, ProtocolDialect, RoutingCredentialId};
use any2api_protocol::api::ProtocolContinuationState;
use thiserror::Error;

use super::{
    hash::SessionHasher,
    lease::{Binding, BindingLease, BindingStart},
    shard::{InsertRejection, ShardedBindings},
    target::AffinityTarget,
};
use crate::routing::SchedulerEpoch;

const MAX_BINDINGS: usize = 300_000;

#[derive(Debug)]
pub(crate) struct AffinityRegistry {
    pub(super) hasher: SessionHasher,
    scheduler_epoch: Arc<SchedulerEpoch>,
    pub(super) bindings: ShardedBindings,
}

#[derive(Debug)]
pub(super) enum BindingState {
    Creating {
        version: u64,
        phase: BindingCreationPhase,
    },
    Bound {
        binding: TimedBinding,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BindingCreationPhase {
    Selecting,
    Attempting,
}

#[derive(Clone, Debug)]
pub(super) enum BindingSource {
    Session,
    Continuation(ContinuationLifecycle),
}

#[derive(Clone, Debug)]
pub(super) enum ContinuationLifecycle {
    Pending {
        version: u64,
    },
    Ready {
        state: Option<ProtocolContinuationState>,
    },
}

#[derive(Clone, Debug)]
pub(super) struct TimedBinding {
    pub(super) target: AffinityTarget,
    pub(super) last_seen_at: Instant,
    pub(super) source: BindingSource,
}

impl AffinityRegistry {
    #[cfg(test)]
    pub(crate) fn new() -> Arc<Self> {
        Self::with_scheduler_epoch(SchedulerEpoch::new())
    }

    pub(crate) fn with_scheduler_epoch(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        Self::with_limits(scheduler_epoch, MAX_BINDINGS)
    }

    fn with_limits(scheduler_epoch: Arc<SchedulerEpoch>, max_bindings: usize) -> Arc<Self> {
        Arc::new(Self {
            hasher: SessionHasher::new(),
            scheduler_epoch,
            bindings: ShardedBindings::new(
                max_bindings,
                super::continuation::MAX_BRIDGE_CONTINUATION_TOTAL_BYTES,
            ),
        })
    }

    #[cfg(test)]
    pub(super) fn with_max_bindings_for_test(max_bindings: usize) -> Arc<Self> {
        Self::with_limits(SchedulerEpoch::new(), max_bindings)
    }

    #[cfg(test)]
    pub(crate) fn subscribe_scheduler_epoch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.scheduler_epoch.subscribe()
    }

    #[cfg(test)]
    pub(crate) fn continuation_bytes_for_test(&self) -> usize {
        self.bindings.continuation_bytes()
    }

    pub(crate) fn begin_session(
        self: &Arc<Self>,
        dialect: ProtocolDialect,
        route_id: ModelRouteId,
        raw: &str,
        ttl: Duration,
    ) -> Result<BindingStart, AffinityError> {
        let key = self.hasher.session(dialect, route_id, raw);
        let now = Instant::now();
        let mut reclaimed = false;
        loop {
            let mut shard = self.bindings.lock(key);
            if let Some(existing) = shard.get_mut(&key) {
                match existing {
                    BindingState::Bound { binding }
                        if now.saturating_duration_since(binding.last_seen_at) < ttl =>
                    {
                        binding.last_seen_at = now;
                        return Ok(BindingStart::Bound(Binding {
                            target: binding.target.clone(),
                        }));
                    }
                    BindingState::Creating { phase, .. } => {
                        return Ok(BindingStart::Wait(*phase));
                    }
                    _ => {}
                }
                shard.remove(&key);
            }
            shard.reclaim_expired_sample(now, ttl);
            let version = self.bindings.next_version();
            let creating = BindingState::Creating {
                version,
                phase: BindingCreationPhase::Selecting,
            };
            if shard.try_insert(key, creating).is_ok() {
                return Ok(BindingStart::Create(BindingLease {
                    registry: Arc::clone(self),
                    key,
                    version,
                    active: true,
                }));
            }
            // At capacity: release the shard lock before scanning the other
            // shards for expired entries, then retry the insert exactly once.
            drop(shard);
            if reclaimed || !self.bindings.reclaim_for_capacity(now, ttl) {
                return Err(AffinityError::Capacity);
            }
            reclaimed = true;
        }
    }

    pub(crate) fn clear_all(&self) -> usize {
        let mut cleared = 0;
        let mut waiting_state_removed = false;
        // Shards are cleared one at a time; entries inserted concurrently in
        // an already-visited shard survive, matching the pre-shard behavior
        // only approximately.
        self.bindings.for_each_shard(|shard| {
            waiting_state_removed |= shard.iter().any(|(_, entry)| match entry {
                BindingState::Creating { .. } => true,
                BindingState::Bound { binding } => binding.is_pending_continuation(),
            });
            cleared += shard.clear();
        });
        if waiting_state_removed {
            self.wake_waiters();
        }
        cleared
    }

    pub(crate) fn clear_credential(&self, credential_id: RoutingCredentialId) -> usize {
        let mut cleared = 0;
        let mut waiting_state_removed = false;
        // Same per-shard visitation caveat as `clear_all`.
        self.bindings.for_each_shard(|shard| {
            let removed = shard
                .iter()
                .filter_map(|(key, entry)| match entry {
                    BindingState::Bound { binding }
                        if binding.target.credential_id() == credential_id =>
                    {
                        Some((*key, binding.is_pending_continuation()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            for (key, pending) in removed {
                waiting_state_removed |= pending;
                cleared += usize::from(shard.remove(&key).is_some());
            }
        });
        if waiting_state_removed {
            self.wake_waiters();
        }
        cleared
    }

    pub(super) fn wake_waiters(&self) -> u64 {
        self.scheduler_epoch.advance()
    }
}

impl TimedBinding {
    pub(super) fn is_pending_continuation(&self) -> bool {
        matches!(
            &self.source,
            BindingSource::Continuation(ContinuationLifecycle::Pending { .. })
        )
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(crate) enum AffinityError {
    #[error("affinity binding capacity is full")]
    Capacity,
    #[error("affinity identity is already bound to another target")]
    IdentityConflict,
    #[error("affinity creating lease is no longer current")]
    LeaseLost,
    #[error("affinity binding deadline elapsed")]
    DeadlineExceeded,
    #[error("protocol continuation state capacity is full")]
    ContinuationCapacity,
    #[error("protocol continuation state exceeds its per-entry limit")]
    ContinuationTooLarge,
}

impl From<InsertRejection> for AffinityError {
    fn from(rejection: InsertRejection) -> Self {
        match rejection {
            InsertRejection::Entries => Self::Capacity,
            InsertRejection::ContinuationBytes => Self::ContinuationCapacity,
        }
    }
}

#[cfg(test)]
impl AffinityRegistry {
    pub(super) fn full_reclaims_for_test(&self) -> usize {
        self.bindings.full_reclaims()
    }

    /// Rewinds `last_seen_at` on every bound entry so tests can force expiry.
    pub(super) fn stale_bound_entries_for_test(&self, at: Instant) {
        self.bindings.for_each_shard(|shard| {
            for (_, entry) in shard.iter_mut() {
                if let BindingState::Bound { binding } = entry {
                    binding.last_seen_at = at;
                }
            }
        });
    }

    pub(super) fn bound_last_seen_for_test(&self) -> Vec<Instant> {
        let mut seen = Vec::new();
        self.bindings.for_each_shard(|shard| {
            for (_, entry) in shard.iter() {
                if let BindingState::Bound { binding } = entry {
                    seen.push(binding.last_seen_at);
                }
            }
        });
        seen
    }
}
