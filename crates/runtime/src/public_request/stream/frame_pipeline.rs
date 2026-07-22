use std::time::Instant;

use any2api_protocol::api::SseFrame;
use bytes::Bytes;

use super::{GuardedBody, PendingFrame, pending_failure::PendingStreamError};
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
                    if let Err(error) = self.push_frame(frame, deadline) {
                        self.set_pending_error(error);
                    }
                    return true;
                }
                Ok(None) => {}
                Err(_) => {
                    self.set_pending_error(PendingStreamError::invalid_response(
                        "upstream SSE frame was invalid",
                    ));
                    return true;
                }
            }
            let Some(mut chunk) = self.buffered_chunk.take() else {
                return false;
            };
            let take = self.decoder.next_input_limit().min(chunk.len());
            let input = chunk.split_to(take);
            if !chunk.is_empty() {
                self.buffered_chunk = Some(chunk);
            }
            self.decoder.push(&input);
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
            Err(_) => {
                self.set_pending_error(PendingStreamError::invalid_response(
                    "upstream SSE frame was invalid",
                ));
            }
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
        let event = self
            .adapter
            .decode_upstream_event(frame)
            .map_err(|_| PendingStreamError::invalid_response("upstream SSE event was invalid"))?;
        let telemetry = event.telemetry();
        let hard_id = self
            .adapter
            .hard_affinity_id_from_event(self.hard_affinity.operation(), &event)
            .map_err(|_| {
                PendingStreamError::invalid_response("upstream SSE identity was invalid")
            })?;
        let frame = self
            .adapter
            .encode_egress_event(event, &self.public_model)
            .map_err(|_| PendingStreamError::local("upstream SSE event could not be encoded"))?;
        if check_deadline && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(PendingStreamError::timeout());
        }
        self.precommit_budget
            .observe_frame(frame.0.len())
            .map_err(|_| PendingStreamError::budget_exceeded())?;
        if let Some(hard_id) = hard_id {
            let result = match deadline {
                Some(deadline) if check_deadline => {
                    self.hard_affinity.bind_before(&hard_id, deadline)
                }
                _ => self.hard_affinity.bind(&hard_id),
            };
            result.map_err(|error| match error {
                AffinityError::DeadlineExceeded => PendingStreamError::timeout(),
                _ => PendingStreamError::local("upstream SSE identity could not be bound"),
            })?;
        } else if check_deadline && deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(PendingStreamError::timeout());
        }
        self.request_recorder
            .observe_token_usage(telemetry.token_usage);
        self.pending.push_back(PendingFrame {
            bytes: frame.0,
            has_content_delta: telemetry.has_content_delta,
        });
        self.precommit_budget.commit();
        Ok(())
    }
}
