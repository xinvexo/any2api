use std::{fmt, sync::Arc, time::Instant};

use any2api_protocol::api::{MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolContinuationState};

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
        continuation: ProtocolContinuationState,
        deadline: Option<Instant>,
    ) -> Result<(), AffinityError> {
        check_deadline(deadline)?;
        let continuation_bytes = checked_state_bytes(Some(&continuation))?;
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        check_deadline(deadline)?;
        let current = matches!(
            state.entries.get(&self.key),
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
        state.continuation_bytes = state
            .continuation_bytes
            .saturating_sub(MAX_BRIDGE_CONTINUATION_STATE_BYTES)
            .saturating_add(continuation_bytes);
        state.entries.insert(
            self.key,
            BindingState::Bound {
                binding: TimedBinding {
                    target: self.target.clone(),
                    last_seen_at: Instant::now(),
                    source: BindingSource::Continuation(ContinuationLifecycle::Ready {
                        state: Some(continuation),
                    }),
                },
            },
        );
        self.active = false;
        drop(state);
        self.registry.wake_waiters();
        Ok(())
    }
}

impl Drop for ContinuationLease {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self
            .registry
            .state
            .lock()
            .expect("affinity state lock poisoned");
        let current = matches!(
            state.entries.get(&self.key),
            Some(BindingState::Bound { binding })
                if matches!(
                    binding.source,
                    BindingSource::Continuation(ContinuationLifecycle::Pending { version })
                        if version == self.version
                )
        );
        if current {
            state.remove(&self.key);
        }
        drop(state);
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
