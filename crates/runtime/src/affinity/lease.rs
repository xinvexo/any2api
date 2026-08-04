use std::{fmt, sync::Arc, time::Instant};

use super::{
    hash::SessionHash,
    registry::{
        AffinityError, AffinityRegistry, BindingCreationPhase, BindingSource, BindingState,
        TimedBinding,
    },
    target::AffinityTarget,
};

#[derive(Clone, Debug)]
pub(crate) struct Binding {
    pub(super) target: AffinityTarget,
}

impl Binding {
    pub(crate) fn target(&self) -> &AffinityTarget {
        &self.target
    }
}

#[derive(Debug)]
pub(crate) enum BindingStart {
    Create(BindingLease),
    Wait(BindingCreationPhase),
    Bound(Binding),
}

pub(crate) struct BindingLease {
    pub(super) registry: Arc<AffinityRegistry>,
    pub(super) key: SessionHash,
    pub(super) version: u64,
    pub(super) active: bool,
}

impl BindingLease {
    pub(crate) fn mark_attempting(&mut self) -> Result<(), AffinityError> {
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        let Some(BindingState::Creating { version, phase }) = state.entries.get_mut(&self.key)
        else {
            return Err(AffinityError::LeaseLost);
        };
        if *version != self.version {
            return Err(AffinityError::LeaseLost);
        }
        *phase = BindingCreationPhase::Attempting;
        drop(state);
        self.registry.wake_waiters();
        Ok(())
    }

    pub(crate) fn release_for_wait(mut self) -> Result<u64, AffinityError> {
        if !self.remove_current() {
            return Err(AffinityError::LeaseLost);
        }
        Ok(self.registry.wake_waiters())
    }

    pub(crate) fn commit(mut self, target: AffinityTarget) -> Result<(), AffinityError> {
        self.commit_with_deadline(target, None)
    }

    pub(crate) fn commit_before(
        mut self,
        target: AffinityTarget,
        deadline: Instant,
    ) -> Result<(), AffinityError> {
        self.commit_with_deadline(target, Some(deadline))
    }

    fn commit_with_deadline(
        &mut self,
        target: AffinityTarget,
        deadline: Option<Instant>,
    ) -> Result<(), AffinityError> {
        let committed_at = Instant::now();
        if deadline.is_some_and(|deadline| committed_at >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        let matches = matches!(
            state.entries.get(&self.key),
            Some(BindingState::Creating {
                version,
                phase: BindingCreationPhase::Attempting,
            }) if *version == self.version
        );
        if !matches {
            return Err(AffinityError::LeaseLost);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        state.entries.insert(
            self.key,
            BindingState::Bound {
                binding: TimedBinding {
                    target,
                    last_seen_at: committed_at,
                    source: BindingSource::Session,
                },
            },
        );
        self.active = false;
        drop(state);
        self.registry.wake_waiters();
        Ok(())
    }
}

impl BindingLease {
    fn remove_current(&mut self) -> bool {
        if !self.active {
            return false;
        }
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        let matches = matches!(
            state.entries.get(&self.key),
            Some(BindingState::Creating { version, .. }) if *version == self.version
        );
        if matches {
            state.entries.remove(&self.key);
        }
        self.active = false;
        matches
    }
}

impl Drop for BindingLease {
    fn drop(&mut self) {
        if self.remove_current() {
            self.registry.wake_waiters();
        }
    }
}

impl fmt::Debug for BindingLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingLease")
            .field("key", &self.key)
            .field("version", &self.version)
            .field("active", &self.active)
            .finish()
    }
}
