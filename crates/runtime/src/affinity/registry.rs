use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use any2api_domain::{CredentialId, ModelRouteId, ProtocolDialect};
use thiserror::Error;
use tokio::sync::watch;

use super::{
    hash::{SessionHash, SessionHasher},
    lease::{SoftBinding, SoftBindingLease, SoftBindingStart, SoftBindingWait},
    target::AffinityTarget,
};

const MAX_SOFT_BINDINGS: usize = 100_000;
const MAX_HARD_BINDINGS: usize = 200_000;

#[derive(Debug)]
pub(crate) struct AffinityRegistry {
    hasher: SessionHasher,
    pub(super) state: Mutex<AffinityState>,
}

#[derive(Debug, Default)]
pub(super) struct AffinityState {
    next_version: u64,
    pub(super) soft: HashMap<SessionHash, SoftState>,
    pub(super) hard: HashMap<SessionHash, TimedBinding>,
}

#[derive(Debug)]
pub(super) enum SoftState {
    Creating {
        version: u64,
        started_at: Instant,
        changes: watch::Sender<u64>,
    },
    Bound {
        version: u64,
        binding: TimedBinding,
    },
}

#[derive(Clone, Debug)]
pub(super) struct TimedBinding {
    pub(super) target: AffinityTarget,
    pub(super) last_seen_at: Instant,
}

impl AffinityRegistry {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            hasher: SessionHasher::new(),
            state: Mutex::new(AffinityState::default()),
        })
    }

    pub(crate) fn begin_soft(
        self: &Arc<Self>,
        dialect: ProtocolDialect,
        route_id: ModelRouteId,
        raw: &str,
        soft_ttl: Duration,
        creating_ttl: Duration,
    ) -> Result<SoftBindingStart, AffinityError> {
        let key = self.hasher.soft(dialect, route_id, raw);
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        if let Some(existing) = state.soft.get_mut(&key) {
            match existing {
                SoftState::Bound { version, binding }
                    if now.saturating_duration_since(binding.last_seen_at) < soft_ttl =>
                {
                    binding.last_seen_at = now;
                    return Ok(SoftBindingStart::Bound(SoftBinding {
                        key,
                        version: *version,
                        target: binding.target.clone(),
                    }));
                }
                SoftState::Creating {
                    started_at,
                    changes,
                    ..
                } if now.saturating_duration_since(*started_at) < creating_ttl => {
                    return Ok(SoftBindingStart::Wait(SoftBindingWait {
                        changes: changes.subscribe(),
                    }));
                }
                _ => {}
            }
            notify_removed(state.soft.remove(&key));
        }
        ensure_soft_capacity(&mut state, now, soft_ttl, creating_ttl)?;
        let version = state.next_version();
        let (changes, _) = watch::channel(0);
        state.soft.insert(
            key,
            SoftState::Creating {
                version,
                started_at: now,
                changes: changes.clone(),
            },
        );
        Ok(SoftBindingStart::Create(SoftBindingLease {
            registry: Arc::clone(self),
            key,
            version,
            changes,
            active: true,
        }))
    }

    pub(crate) fn invalidate_soft(&self, binding: &SoftBinding) -> bool {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let matches = matches!(
            state.soft.get(&binding.key),
            Some(SoftState::Bound { version, .. }) if *version == binding.version
        );
        if matches {
            state.soft.remove(&binding.key);
        }
        matches
    }

    pub(crate) fn resolve_hard(&self, raw: &str, ttl: Duration) -> Option<AffinityTarget> {
        let key = self.hasher.hard(raw);
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let binding = state.hard.get_mut(&key)?;
        if now.saturating_duration_since(binding.last_seen_at) >= ttl {
            state.hard.remove(&key);
            return None;
        }
        binding.last_seen_at = now;
        Some(binding.target.clone())
    }

    pub(crate) fn bind_hard(
        &self,
        raw: &str,
        target: AffinityTarget,
        ttl: Duration,
    ) -> Result<(), AffinityError> {
        let key = self.hasher.hard(raw);
        let now = Instant::now();
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        if let Some(binding) = state.hard.get_mut(&key) {
            if binding.target != target {
                return Err(AffinityError::IdentityConflict);
            }
            binding.last_seen_at = now;
            return Ok(());
        }
        if state.hard.len() >= MAX_HARD_BINDINGS {
            state
                .hard
                .retain(|_, binding| now.saturating_duration_since(binding.last_seen_at) < ttl);
        }
        if state.hard.len() >= MAX_HARD_BINDINGS {
            return Err(AffinityError::Capacity);
        }
        state.hard.insert(
            key,
            TimedBinding {
                target,
                last_seen_at: now,
            },
        );
        Ok(())
    }

    pub(crate) fn retain_credentials(&self, active: &HashSet<CredentialId>) {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        state
            .hard
            .retain(|_, binding| active.contains(&binding.target.credential_id()));
        state.soft.retain(|_, binding| match binding {
            SoftState::Bound { binding, .. } => active.contains(&binding.target.credential_id()),
            SoftState::Creating { .. } => true,
        });
    }

    pub(crate) fn clear_all(&self) -> usize {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let cleared = state.soft.len() + state.hard.len();
        for binding in state.soft.drain().map(|(_, binding)| binding) {
            notify_removed(Some(binding));
        }
        state.hard.clear();
        cleared
    }

    pub(crate) fn clear_credential(&self, credential_id: CredentialId) -> usize {
        let mut state = self.state.lock().expect("affinity state lock poisoned");
        let before = state.soft.len() + state.hard.len();
        state
            .hard
            .retain(|_, binding| binding.target.credential_id() != credential_id);
        state.soft.retain(|_, binding| match binding {
            SoftState::Bound { binding, .. } => binding.target.credential_id() != credential_id,
            SoftState::Creating { .. } => true,
        });
        before - state.soft.len() - state.hard.len()
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
}

fn ensure_soft_capacity(
    state: &mut AffinityState,
    now: Instant,
    soft_ttl: Duration,
    creating_ttl: Duration,
) -> Result<(), AffinityError> {
    if state.soft.len() < MAX_SOFT_BINDINGS {
        return Ok(());
    }
    state.soft.retain(|_, binding| match binding {
        SoftState::Creating {
            started_at,
            changes,
            ..
        } => {
            let keep = now.saturating_duration_since(*started_at) < creating_ttl;
            if !keep {
                changes.send_replace(1);
            }
            keep
        }
        SoftState::Bound { binding, .. } => {
            now.saturating_duration_since(binding.last_seen_at) < soft_ttl
        }
    });
    (state.soft.len() < MAX_SOFT_BINDINGS)
        .then_some(())
        .ok_or(AffinityError::Capacity)
}

fn notify_removed(binding: Option<SoftState>) {
    if let Some(SoftState::Creating { changes, .. }) = binding {
        changes.send_replace(1);
    }
}
