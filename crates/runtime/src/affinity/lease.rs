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
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        let matches = matches!(
            state.soft.get(&self.key),
            Some(SoftState::Creating { version, .. }) if *version == self.version
        );
        if !matches {
            return Err(AffinityError::LeaseLost);
        }
        state.soft.insert(
            self.key,
            SoftState::Bound {
                version: self.version,
                binding: TimedBinding {
                    target,
                    last_seen_at: Instant::now(),
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
