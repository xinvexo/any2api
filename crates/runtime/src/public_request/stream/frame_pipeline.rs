use std::time::Instant;

use any2api_protocol::api::{
    BridgeContinuationState, SseFrame, StreamCompletionPolicy, StreamTermination,
};
use bytes::Bytes;

use super::{
    GuardedBody, PendingFrame, PrecommitUpstreamFailure, pending_failure::PendingStreamError,
};
use crate::affinity::AffinityError;

impl GuardedBody {
    pub(super) fn process_chunk(&mut self, chunk: Bytes, deadline: Option<Instant>) {
        debug_assert!(self.buffered_chunk.is_none());
        if !chunk.is_empty() {
            self.buffered_chunk = Some(chunk);
        }
        self.process_buffered_frame(deadline);
    }

    pub(super) fn process_eof(&mut self, deadline: Option<Instant>) {
        self.upstream_done = true;
        if self.process_buffered_frame(deadline) {
            return;
        }
        self.finish_decoder(deadline);
    }

    pub(super) fn process_buffered_frame(&mut self, deadline: Option<Instant>) -> bool {
        loop {
            match self.decoder.next_frame() {
                Ok(Some(frame)) => {
                    if let Some(recorder) = self.attempt_recorder.as_mut() {
                        recorder.observe_first_upstream_frame();
                    }
                    if let Err(error) = self.push_frame(frame, deadline) {
                        self.set_pending_error(error);
                    }
                    return true;
                }
                Ok(None) => {}
                Err(error) => {
                    self.set_pending_error(PendingStreamError::invalid_response(format!(
                        "upstream SSE frame was invalid: {error}"
                    )));
                    return true;
                }
            }
            let Some(mut chunk) = self.buffered_chunk.take() else {
                return false;
            };
            let take = self.decoder.next_input_limit().min(chunk.len());
            let consumed = match self.decoder.push(&chunk[..take]) {
                Ok(consumed) => consumed,
                Err(error) => {
                    self.set_pending_error(PendingStreamError::invalid_response(format!(
                        "upstream SSE frame was invalid: {error}"
                    )));
                    return true;
                }
            };
            chunk = chunk.slice(consumed..);
            if !chunk.is_empty() {
                self.buffered_chunk = Some(chunk);
            }
        }
    }

    pub(super) fn finish_decoder(&mut self, deadline: Option<Instant>) {
        if self.decoder_finished {
            return;
        }
        self.decoder_finished = true;
        match self.decoder.finish() {
            Ok(Some(frame)) => {
                if let Err(error) = self.push_frame(frame, deadline) {
                    self.set_pending_error(error);
                }
            }
            Ok(None) => {}
            Err(error) => {
                self.set_pending_error(PendingStreamError::invalid_response(format!(
                    "upstream SSE frame was invalid: {error}"
                )));
            }
        }
        if self.pending_error.is_none() {
            match self.exchange.finish_upstream_events() {
                Ok(events) => {
                    for event in events {
                        if let Err(error) = self.push_event(event, deadline) {
                            self.set_pending_error(error);
                            break;
                        }
                        if self.terminal_seen {
                            break;
                        }
                    }
                }
                Err(_) => self.set_pending_error(PendingStreamError::invalid_response(
                    "upstream SSE bridge could not be finalized",
                )),
            }
        }
        if self.pending_error.is_none()
            && self.exchange.stream_completion_policy()
                == StreamCompletionPolicy::TerminalEventRequired
            && !self.terminal_seen
        {
            self.set_pending_error(PendingStreamError::invalid_response(
                "upstream SSE ended before a required terminal event",
            ));
        }
    }

    fn push_frame(
        &mut self,
        frame: SseFrame,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        let check_deadline = !self.precommit_budget.is_committed();
        if check_deadline && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(PendingStreamError::timeout());
        }
        let events = self
            .exchange
            .decode_upstream_event(frame)
            .map_err(|_| PendingStreamError::invalid_response("upstream SSE event was invalid"))?;
        for event in events {
            self.push_event(event, deadline)?;
            if self.terminal_seen {
                break;
            }
        }
        Ok(())
    }

    fn push_event(
        &mut self,
        event: any2api_protocol::api::AdapterEvent,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        let check_deadline = !self.precommit_budget.is_committed();
        if check_deadline && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(PendingStreamError::timeout());
        }
        let telemetry = event.telemetry();
        let termination = event.termination();
        let allow_retry_safety_override = check_deadline && !self.precommit_commit_ready;
        let upstream_failure = event.upstream_failure().map(|evidence| {
            (
                self.classify_upstream_failure(evidence, allow_retry_safety_override),
                evidence.retry_delay_basis(),
            )
        });
        let continuation_id = self
            .exchange
            .continuation_id_from_event(self.continuation_binding.operation(), &event)
            .map_err(|_| {
                PendingStreamError::invalid_response("upstream SSE identity was invalid")
            })?;
        let continuation_state = self.exchange.bridge_continuation_state();
        if termination == StreamTermination::Completed
            && matches!(continuation_state, BridgeContinuationState::Pending)
        {
            return Err(PendingStreamError::invalid_response(
                "upstream SSE continuation was incomplete",
            ));
        }
        let frame = self
            .exchange
            .encode_egress_event(event, &self.public_model)
            .map_err(|_| PendingStreamError::local("upstream SSE event could not be encoded"))?;
        if check_deadline && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(PendingStreamError::timeout());
        }
        self.precommit_budget
            .observe_frame(frame.0.len())
            .map_err(|_| PendingStreamError::budget_exceeded())?;
        if check_deadline {
            self.validate_deferred_continuation_id(continuation_id.as_deref())?;
        } else {
            self.commit_continuation_state(
                continuation_id.as_deref(),
                continuation_state.clone(),
                None,
            )?;
            self.request_recorder
                .observe_token_usage(telemetry.token_usage);
        }
        let frame = frame.0;
        if check_deadline
            && termination == StreamTermination::Failed
            && !self.precommit_commit_ready
            && let Some((error, retry_delay_basis)) = upstream_failure.as_ref()
        {
            self.precommit_upstream_failure = Some(PrecommitUpstreamFailure {
                error: error.clone(),
                frame: frame.clone(),
                retry_delay_basis: *retry_delay_basis,
            });
        }
        self.pending.push_back(PendingFrame {
            bytes: frame,
            has_content_delta: telemetry.has_content_delta,
            termination,
            token_usage: telemetry.token_usage,
            continuation_id,
            continuation_state,
            upstream_error: upstream_failure.map(|(error, _)| error),
        });
        self.terminal_seen |= termination.is_terminal();
        if check_deadline
            && self.precommit_upstream_failure.is_none()
            && (telemetry.has_content_delta
                || !telemetry.retry_transparent
                || termination.is_terminal())
        {
            self.precommit_commit_ready = true;
        }
        Ok(())
    }

    pub(super) fn commit_precommit_frames(
        &mut self,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        debug_assert!(!self.precommit_budget.is_committed());
        let mut frames = std::mem::take(&mut self.pending);
        while let Some(mut frame) = frames.pop_front() {
            let continuation_id = frame.continuation_id.take();
            let continuation_state = std::mem::replace(
                &mut frame.continuation_state,
                BridgeContinuationState::Stateless,
            );
            self.stage_precommit_continuation_state(
                continuation_id.as_deref(),
                continuation_state,
                deadline,
            )?;
            self.request_recorder.observe_token_usage(frame.token_usage);
            self.pending.push_back(frame);
        }
        self.precommit_budget.commit();
        if let Some(recorder) = self.attempt_recorder.as_mut() {
            recorder.observe_stream_commit();
        }
        Ok(())
    }

    fn validate_deferred_continuation_id(
        &self,
        continuation_id: Option<&str>,
    ) -> Result<(), PendingStreamError> {
        let Some(id) = continuation_id else {
            return Ok(());
        };
        let conflicts_with_committed = self
            .continuation_id
            .as_deref()
            .is_some_and(|existing| existing != id);
        let conflicts_with_pending = self
            .pending
            .iter()
            .filter_map(|frame| frame.continuation_id.as_deref())
            .any(|existing| existing != id);
        if conflicts_with_committed || conflicts_with_pending {
            return Err(PendingStreamError::local(
                "upstream SSE emitted conflicting response identities",
            ));
        }
        Ok(())
    }

    fn commit_continuation_state(
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
        match state {
            BridgeContinuationState::Stateless => {
                let Some(id) = continuation_id else {
                    return Ok(());
                };
                self.bind_ready(id, None, deadline)?;
                self.continuation_id = Some(id.to_owned());
            }
            BridgeContinuationState::Pending => {
                let Some(id) = continuation_id else {
                    return Ok(());
                };
                if self.continuation_lease.is_none() {
                    let lease = match deadline {
                        Some(deadline) => {
                            self.continuation_binding.begin_pending_before(id, deadline)
                        }
                        None => self.continuation_binding.begin_pending(id),
                    }
                    .map_err(binding_error)?;
                    self.continuation_lease = Some(lease);
                    self.continuation_id = Some(id.to_owned());
                }
            }
            BridgeContinuationState::Ready(continuation) => {
                if let Some(lease) = self.continuation_lease.as_mut() {
                    lease
                        .ready(Some(continuation), deadline)
                        .map_err(binding_error)?;
                    self.continuation_lease.take();
                } else if let Some(id) = continuation_id {
                    self.bind_ready(id, Some(continuation), deadline)?;
                    self.continuation_id = Some(id.to_owned());
                } else if self.continuation_id.is_none() {
                    return Err(PendingStreamError::local(
                        "upstream SSE continuation identity was missing",
                    ));
                }
            }
        }
        Ok(())
    }

    fn bind_ready(
        &self,
        id: &str,
        state: Option<any2api_protocol::api::ProtocolContinuationState>,
        deadline: Option<Instant>,
    ) -> Result<(), PendingStreamError> {
        match deadline {
            Some(deadline) => self
                .continuation_binding
                .bind_ready_before(id, state, deadline),
            None => self.continuation_binding.bind_ready(id, state),
        }
        .map_err(binding_error)
    }
}

pub(super) fn binding_error(error: AffinityError) -> PendingStreamError {
    match error {
        AffinityError::DeadlineExceeded => PendingStreamError::timeout(),
        AffinityError::Capacity | AffinityError::ContinuationCapacity => {
            PendingStreamError::local("protocol continuation capacity is full")
        }
        AffinityError::ContinuationTooLarge => {
            PendingStreamError::local("protocol continuation state is too large")
        }
        AffinityError::IdentityConflict | AffinityError::LeaseLost => {
            PendingStreamError::local("upstream SSE identity could not be bound")
        }
    }
}
