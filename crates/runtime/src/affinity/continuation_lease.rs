use std::{fmt, sync::Arc, time::Instant};

use any2api_protocol::api::ProtocolContinuationState;

use super::{
    continuation::{check_deadline, checked_state_bytes},
    hash::SessionHash,
    registry::{
        AffinityError, AffinityRegistry, BindingSource, BindingState, ContinuationLifecycle,
        TimedBinding,
    },
    target::AffinityTarget,
};

pub(crate) struct ContinuationLease {
    pub(super) registry: Arc<AffinityRegistry>,
    pub(super) key: SessionHash,
    pub(super) version: u64,
    pub(super) target: AffinityTarget,
    pub(super) active: bool,
}

impl ContinuationLease {
    pub(crate) fn ready(
        &mut self,
        continuation: Option<ProtocolContinuationState>,
        deadline: Option<Instant>,
    ) -> Result<(), AffinityError> {
        check_deadline(deadline)?;
        checked_state_bytes(continuation.as_ref())?;
        let mut shard = self.registry.bindings.lock(self.key);
        check_deadline(deadline)?;
        let current = matches!(
            shard.get(&self.key),
            Some(BindingState::Bound { binding })
                if binding.target == self.target
                    && matches!(
                        binding.source,
                        BindingSource::Continuation(ContinuationLifecycle::Pending { version })
                            if version == self.version
                    )
        );
        if !current {
            return Err(AffinityError::LeaseLost);
        }
        shard.replace(
            self.key,
            BindingState::Bound {
                binding: TimedBinding {
                    target: self.target.clone(),
                    last_seen_at: Instant::now(),
                    source: BindingSource::Continuation(ContinuationLifecycle::Ready {
                        state: continuation,
                    }),
                },
            },
        );
        self.active = false;
        drop(shard);
        self.registry.wake_waiters();
        Ok(())
    }
}

impl Drop for ContinuationLease {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut shard = self.registry.bindings.lock(self.key);
        let current = matches!(
            shard.get(&self.key),
            Some(BindingState::Bound { binding })
                if matches!(
                    binding.source,
                    BindingSource::Continuation(ContinuationLifecycle::Pending { version })
                        if version == self.version
                )
        );
        if current {
            shard.remove(&self.key);
        }
        drop(shard);
        if current {
            self.registry.wake_waiters();
        }
    }
}

impl fmt::Debug for ContinuationLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContinuationLease")
            .field("key", &self.key)
            .field("version", &self.version)
            .field("target", &self.target)
            .field("active", &self.active)
            .finish()
    }
}
