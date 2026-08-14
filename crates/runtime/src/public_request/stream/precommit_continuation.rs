use std::time::Instant;

use any2api_protocol::api::BridgeContinuationState;

use super::{
    GuardedBody, body::PrecommitContinuation, frame_pipeline::binding_error,
    pending_failure::PendingStreamError,
};

impl GuardedBody {
    pub(super) fn stage_precommit_continuation_state(
        &mut self,
        continuation_id: Option<&str>,
        state: BridgeContinuationState,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        if let Some(id) = continuation_id
            && self
                .continuation_id
                .as_deref()
                .is_some_and(|existing| existing != id)
        {
            return Err(PendingStreamError::local(
                "upstream SSE emitted conflicting response identities",
            ));
        }
        if self.precommit_continuation.is_none() && continuation_id.is_none() {
            return match state {
                BridgeContinuationState::Stateless | BridgeContinuationState::Pending => Ok(()),
                BridgeContinuationState::Ready(_) => Err(PendingStreamError::local(
                    "upstream SSE continuation identity was missing",
                )),
            };
        }
        let next = match state {
            BridgeContinuationState::Stateless => PrecommitContinuation::Ready(None),
            BridgeContinuationState::Pending => PrecommitContinuation::Pending,
            BridgeContinuationState::Ready(state) => PrecommitContinuation::Ready(Some(state)),
        };
        match (&self.precommit_continuation, &next) {
            (None, _)
            | (
                Some(PrecommitContinuation::Pending),
                PrecommitContinuation::Pending | PrecommitContinuation::Ready(Some(_)),
            ) => {}
            (Some(PrecommitContinuation::Ready(None)), PrecommitContinuation::Ready(None))
            | (
                Some(PrecommitContinuation::Ready(Some(_))),
                PrecommitContinuation::Ready(Some(_)),
            ) => return Ok(()),
            _ => {
                return Err(PendingStreamError::local(
                    "upstream SSE emitted conflicting continuation states",
                ));
            }
        }
        if let Some(id) = continuation_id {
            self.begin_precommit_continuation(id, deadline)?;
        }
        if self.continuation_lease.is_none() {
            return Err(PendingStreamError::local(
                "upstream SSE continuation identity was missing",
            ));
        }
        self.precommit_continuation = Some(next);
        Ok(())
    }

    fn begin_precommit_continuation(
        &mut self,
        id: &str,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        if self.continuation_lease.is_some() {
            return Ok(());
        }
        let lease = match deadline {
            Some(deadline) => self.continuation_binding.begin_pending_before(id, deadline),
            None => self.continuation_binding.begin_pending(id),
        }
        .map_err(binding_error)?;
        self.continuation_lease = Some(lease);
        self.continuation_id = Some(id.to_owned());
        Ok(())
    }

    pub(super) fn finalize_precommit_continuation(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        let state = match self.precommit_continuation.take() {
            None | Some(PrecommitContinuation::Pending) => return Ok(()),
            Some(PrecommitContinuation::Ready(state)) => state,
        };
        let lease = self.continuation_lease.as_mut().ok_or_else(|| {
            PendingStreamError::local("upstream SSE continuation lease was missing")
        })?;
        lease.ready(state, deadline).map_err(binding_error)?;
        self.continuation_lease.take();
        Ok(())
    }
}
