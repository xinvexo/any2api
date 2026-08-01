use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use any2api_domain::{ModelRouteId, ProtocolDialect, RoutingCredentialId};
use any2api_protocol::api::{MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolContinuationState};
use thiserror::Error;

use super::{
    hash::{SessionHash, SessionHasher},
    lease::{Binding, BindingLease, BindingStart},
    target::AffinityTarget,
};
use crate::routing::SchedulerEpoch;

const MAX_BINDINGS: usize = 300_000;

#[derive(Debug)]
pub(crate) struct AffinityRegistry {
    pub(super) hasher: SessionHasher,
    scheduler_epoch: Arc<SchedulerEpoch>,
    pub(super) state: Mutex<AffinityState>,
}

#[derive(Debug, Default)]
pub(super) struct AffinityState {
    next_version: u64,
    pub(super) continuation_bytes: usize,
    pub(super) entries: HashMap<SessionHash, BindingState>,
}

#[derive(Debug)]
pub(super) enum BindingState {
    Creating { version: u64 },
    Bound { binding: TimedBinding },
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
        Arc::new(Self {
            hasher: SessionHasher::new(),
            scheduler_epoch,
            state: Mutex::new(AffinityState::default()),
        })
    }

    #[cfg(test)]
    pub(crate) fn subscribe_scheduler_epoch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.scheduler_epoch.subscribe()
    }

    #[cfg(test)]
    pub(crate) fn continuation_bytes_for_test(&self) -> usize {
        self.state
            .lock()
            .expect("affinity state lock poisoned")
            .continuation_bytes
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
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        if let Some(existing) = state.entries.get_mut(&key) {
            match existing {
                BindingState::Bound { binding }
                    if now.saturating_duration_since(binding.last_seen_at) < ttl =>
                {
                    binding.last_seen_at = now;
                    return Ok(BindingStart::Bound(Binding {
                        target: binding.target.clone(),
                    }));
                }
                BindingState::Creating { .. } => return Ok(BindingStart::Wait),
                _ => {}
            }
            state.remove(&key);
        }
        ensure_capacity(&mut state, now, ttl)?;
        let version = state.next_version();
        state
            .entries
            .insert(key, BindingState::Creating { version });
        Ok(BindingStart::Create(BindingLease {
            registry: Arc::clone(self),
            key,
            version,
            active: true,
        }))
    }

    pub(crate) fn clear_all(&self) -> usize {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let cleared = state.entries.len();
        let waiting_state_removed = state.entries.values().any(|entry| match entry {
            BindingState::Creating { .. } => true,
            BindingState::Bound { binding } => binding.is_pending_continuation(),
        });
        state.entries.clear();
        state.continuation_bytes = 0;
        drop(state);
        if waiting_state_removed {
            self.wake_waiters();
        }
        cleared
    }

    pub(crate) fn clear_credential(&self, credential_id: RoutingCredentialId) -> usize {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let before = state.entries.len();
        let removed = state
            .entries
            .iter()
            .filter_map(|(key, entry)| match entry {
                BindingState::Bound { binding }
                    if binding.target.credential_id() == credential_id =>
                {
                    Some(*key)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let waiting_state_removed = removed.iter().any(|key| {
            matches!(
                state.entries.get(key),
                Some(BindingState::Bound { binding }) if binding.is_pending_continuation()
            )
        });
        for key in removed {
            state.remove(&key);
        }
        let cleared = before - state.entries.len();
        drop(state);
        if waiting_state_removed {
            self.wake_waiters();
        }
        cleared
    }

    pub(super) fn wake_waiters(&self) {
        self.scheduler_epoch.advance();
    }
}

impl AffinityState {
    pub(super) fn next_version(&mut self) -> u64 {
        self.next_version = self.next_version.wrapping_add(1).max(1);
        self.next_version
    }

    pub(super) fn remove(&mut self, key: &SessionHash) -> Option<BindingState> {
        let removed = self.entries.remove(key)?;
        self.continuation_bytes = self
            .continuation_bytes
            .saturating_sub(continuation_bytes(&removed));
        Some(removed)
    }

    pub(super) fn remove_expired(&mut self, now: Instant, ttl: Duration) -> usize {
        let expired = self
            .entries
            .iter()
            .filter_map(|(key, entry)| match entry {
                BindingState::Bound { binding }
                    if !binding.is_pending_continuation()
                        && now.saturating_duration_since(binding.last_seen_at) >= ttl =>
                {
                    Some(*key)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let removed = expired.len();
        for key in expired {
            self.remove(&key);
        }
        removed
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

pub(super) fn ensure_capacity(
    state: &mut AffinityState,
    now: Instant,
    ttl: Duration,
) -> Result<(), AffinityError> {
    if state.entries.len() < MAX_BINDINGS {
        return Ok(());
    }
    state.remove_expired(now, ttl);
    (state.entries.len() < MAX_BINDINGS)
        .then_some(())
        .ok_or(AffinityError::Capacity)
}

fn continuation_bytes(entry: &BindingState) -> usize {
    let BindingState::Bound { binding } = entry else {
        return 0;
    };
    match &binding.source {
        BindingSource::Session => 0,
        BindingSource::Continuation(ContinuationLifecycle::Pending { .. }) => {
            MAX_BRIDGE_CONTINUATION_STATE_BYTES
        }
        BindingSource::Continuation(ContinuationLifecycle::Ready { state }) => state
            .as_ref()
            .map_or(0, ProtocolContinuationState::serialized_bytes),
    }
}
