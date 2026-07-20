use std::{fmt, sync::Arc, time::Instant};

use tokio::sync::watch;

use super::{
    hash::SessionHash,
    registry::{AffinityError, AffinityRegistry, SoftState, TimedBinding},
    target::AffinityTarget,
};

#[derive(Clone, Debug)]
pub(crate) struct SoftBinding {
    pub(super) key: SessionHash,
    pub(super) version: u64,
    pub(super) target: AffinityTarget,
}

impl SoftBinding {
    pub(crate) fn target(&self) -> &AffinityTarget {
        &self.target
    }
}

#[derive(Debug)]
pub(crate) struct SoftBindingWait {
    pub(super) changes: watch::Receiver<u64>,
}

impl SoftBindingWait {
    pub(crate) async fn changed(&mut self) -> Result<(), watch::error::RecvError> {
        self.changes.changed().await
    }
}

#[derive(Debug)]
pub(crate) enum SoftBindingStart {
    Create(SoftBindingLease),
    Wait(SoftBindingWait),
    Bound(SoftBinding),
}

pub(crate) struct SoftBindingLease {
    pub(super) registry: Arc<AffinityRegistry>,
    pub(super) key: SessionHash,
    pub(super) version: u64,
    pub(super) changes: watch::Sender<u64>,
    pub(super) active: bool,
}

impl SoftBindingLease {
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
            state.soft.get(&self.key),
            Some(SoftState::Creating { version, .. }) if *version == self.version
        );
        if !matches {
            return Err(AffinityError::LeaseLost);
        }
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(AffinityError::DeadlineExceeded);
        }
        state.soft.insert(
            self.key,
            SoftState::Bound {
                version: self.version,
                binding: TimedBinding {
                    target,
                    last_seen_at: committed_at,
                },
            },
        );
        self.changes.send_replace(1);
        self.active = false;
        Ok(())
    }
}

impl Drop for SoftBindingLease {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        let matches = matches!(
            state.soft.get(&self.key),
            Some(SoftState::Creating { version, .. }) if *version == self.version
        );
        if matches {
            state.soft.remove(&self.key);
            self.changes.send_replace(1);
        }
    }
}

impl fmt::Debug for SoftBindingLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SoftBindingLease")
            .field("key", &self.key)
            .field("version", &self.version)
            .field("active", &self.active)
            .finish()
    }
}
