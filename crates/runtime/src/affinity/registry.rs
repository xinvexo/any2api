use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use any2api_domain::{ModelRouteId, ProtocolDialect, RoutingCredentialId};
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
    hasher: SessionHasher,
    scheduler_epoch: Arc<SchedulerEpoch>,
    pub(super) state: Mutex<AffinityState>,
}

#[derive(Debug, Default)]
pub(super) struct AffinityState {
    next_version: u64,
    active_credentials: Option<HashSet<RoutingCredentialId>>,
    pub(super) entries: HashMap<SessionHash, BindingState>,
}

#[derive(Debug)]
pub(super) enum BindingState {
    Creating { version: u64 },
    Bound { binding: TimedBinding },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BindingSource {
    Session,
    Continuation,
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
            state.entries.remove(&key);
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

    pub(crate) fn resolve_continuation(&self, raw: &str, ttl: Duration) -> Option<AffinityTarget> {
        let key = self.hasher.continuation(raw);
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let BindingState::Bound { binding } = state.entries.get_mut(&key)? else {
            return None;
        };
        if now.saturating_duration_since(binding.last_seen_at) >= ttl {
            state.entries.remove(&key);
            return None;
        }
        binding.last_seen_at = now;
        Some(binding.target.clone())
    }

    pub(crate) fn bind_continuation(
        &self,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
    ) -> Result<(), AffinityError> {
        self.bind_continuation_with_deadline(raw, target, ttl, None)
    }

    pub(crate) fn bind_continuation_before(
        &self,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
        deadline: Instant,
    ) -> Result<(), AffinityError> {
        self.bind_continuation_with_deadline(raw, target, ttl, Some(deadline))
    }

    fn bind_continuation_with_deadline(
        &self,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
        deadline: Option<Instant>,
    ) -> Result<(), AffinityError> {
        let key = self.hasher.continuation(raw);
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        if !target_is_active(&state, &target) {
            return Err(AffinityError::TargetInactive);
        }
        let now = Instant::now();
        let expired = matches!(
            state.entries.get(&key),
            Some(BindingState::Bound { binding })
                if now.saturating_duration_since(binding.last_seen_at) >= ttl
        );
        if expired {
            state.entries.remove(&key);
        }
        if let Some(existing) = state.entries.get_mut(&key) {
            let BindingState::Bound { binding } = existing else {
                return Err(AffinityError::IdentityConflict);
            };
            if binding.target != target {
                return Err(AffinityError::IdentityConflict);
            }
            let committed_at = Instant::now();
            if deadline.is_some_and(|deadline| committed_at >= deadline) {
                return Err(AffinityError::DeadlineExceeded);
            }
            binding.last_seen_at = committed_at;
            return Ok(());
        }
        ensure_capacity(&mut state, Instant::now(), ttl)?;
        let committed_at = Instant::now();
        if deadline.is_some_and(|deadline| committed_at >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        state.entries.insert(
            key,
            BindingState::Bound {
                binding: TimedBinding {
                    target,
                    last_seen_at: committed_at,
                    source: BindingSource::Continuation,
                },
            },
        );
        Ok(())
    }

    pub(crate) fn retain_credentials(&self, active: &HashSet<RoutingCredentialId>) {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        state.active_credentials = Some(active.clone());
        state.entries.retain(|_, entry| match entry {
            BindingState::Bound { binding } => active.contains(&binding.target.credential_id()),
            BindingState::Creating { .. } => true,
        });
    }

    pub(crate) fn clear_all(&self) -> usize {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let cleared = state.entries.len();
        let creating_removed = state
            .entries
            .values()
            .any(|entry| matches!(entry, BindingState::Creating { .. }));
        state.entries.clear();
        drop(state);
        if creating_removed {
            self.wake_waiters();
        }
        cleared
    }

    pub(crate) fn clear_credential(&self, credential_id: RoutingCredentialId) -> usize {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let before = state.entries.len();
        state.entries.retain(|_, entry| match entry {
            BindingState::Bound { binding } => binding.target.credential_id() != credential_id,
            BindingState::Creating { .. } => true,
        });
        before - state.entries.len()
    }

    pub(super) fn wake_waiters(&self) {
        self.scheduler_epoch.advance();
    }
}

impl AffinityState {
    fn next_version(&mut self) -> u64 {
        self.next_version = self.next_version.wrapping_add(1).max(1);
        self.next_version
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
    #[error("affinity target credential is no longer active")]
    TargetInactive,
}

fn ensure_capacity(
    state: &mut AffinityState,
    now: Instant,
    ttl: Duration,
) -> Result<(), AffinityError> {
    if state.entries.len() < MAX_BINDINGS {
        return Ok(());
    }
    state.entries.retain(|_, entry| match entry {
        BindingState::Creating { .. } => true,
        BindingState::Bound { binding } => {
            now.saturating_duration_since(binding.last_seen_at) < ttl
        }
    });
    (state.entries.len() < MAX_BINDINGS)
        .then_some(())
        .ok_or(AffinityError::Capacity)
}

pub(super) fn target_is_active(state: &AffinityState, target: &AffinityTarget) -> bool {
    state
        .active_credentials
        .as_ref()
        .is_none_or(|active| active.contains(&target.credential_id()))
}
