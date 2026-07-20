mod pending_failure;

use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use any2api_domain::{ErrorClass, PublicError, PublicErrorCode};
use any2api_protocol::{
    SseDecoder,
    api::{ProtocolAdapter, SseFrame},
};
use any2api_transport::api::{BoxByteStream, TransportFailureScope};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use tokio::time::timeout;

use self::pending_failure::{PendingStreamError, PendingStreamErrorKind, transport_error_class};
use super::{PublicResponseStream, RequestPermit, response::public_error};
use crate::request_telemetry::{AttemptRecorder, RequestRecorder};
use crate::{affinity::HardAffinityCommitter, health::AttemptHealth};

const PRECOMMIT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Default)]
pub(super) struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CommitState {
    Pending,
    TransportCommitted,
    Finished,
}

pub(super) struct GuardedBodyParts {
    pub(super) permit: RequestPermit,
    pub(super) health: Option<AttemptHealth>,
    pub(super) hard_affinity: HardAffinityCommitter,
    pub(super) attempt_recorder: AttemptRecorder,
    pub(super) status_code: u16,
}

pub(super) struct GuardedBody {
    upstream: BoxByteStream,
    decoder: SseDecoder,
    adapter: Arc<dyn ProtocolAdapter>,
    public_model: String,
    pending: VecDeque<Bytes>,
    pending_error: Option<PendingStreamError>,
    permit: Option<RequestPermit>,
    health: Option<AttemptHealth>,
    hard_affinity: HardAffinityCommitter,
    cancellation: CancellationToken,
    state: CommitState,
    upstream_done: bool,
    attempt_recorder: Option<AttemptRecorder>,
    request_recorder: RequestRecorder,
    status_code: u16,
    owns_request_completion: bool,
}

impl GuardedBody {
    pub(super) fn new(
        upstream: BoxByteStream,
        adapter: Arc<dyn ProtocolAdapter>,
        public_model: impl Into<String>,
        parts: GuardedBodyParts,
    ) -> Self {
        let GuardedBodyParts {
            permit,
            health,
            hard_affinity,
            attempt_recorder,
            status_code,
        } = parts;
        let request_recorder = attempt_recorder.request();
        Self {
            upstream,
            decoder: SseDecoder::default(),
            adapter,
            public_model: public_model.into(),
            pending: VecDeque::new(),
            pending_error: None,
            permit: Some(permit),
            health,
            hard_affinity,
            cancellation: CancellationToken::default(),
            state: CommitState::Pending,
            upstream_done: false,
            attempt_recorder: Some(attempt_recorder),
            request_recorder,
            status_code,
            owns_request_completion: false,
        }
    }

    pub(super) async fn prime(mut self) -> Result<Self, PublicError> {
        let deadline = Instant::now() + PRECOMMIT_TIMEOUT;
        while self.pending.is_empty() && self.pending_error.is_none() && !self.upstream_done {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.set_timeout_error();
                break;
            }
            match timeout(remaining, self.upstream.next()).await {
                Ok(Some(Ok(chunk))) => self.process_chunk(chunk),
                Ok(Some(Err(error))) => {
                    self.set_transport_error(&error);
                }
                Ok(None) => self.process_eof(),
                Err(_) => {
                    self.set_timeout_error();
                }
            }
        }
        if self.pending.is_empty() {
            return Err(self.finish_precommit_failure());
        }
        if let Some(health) = self.health.take() {
            health.success();
        }
        Ok(self)
    }

    pub(super) fn fail_before_handoff(&mut self, error_class: ErrorClass) {
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.local_error(Some(self.status_code), error_class);
        }
        self.release_guards();
    }

    pub(super) fn into_stream(mut self) -> PublicResponseStream {
        self.owns_request_completion = true;
        Box::pin(self)
    }

    fn process_chunk(&mut self, chunk: Bytes) {
        let frames = match self.decoder.push(&chunk) {
            Ok(frames) => frames,
            Err(_) => {
                self.set_pending_error(PendingStreamError::invalid_response(
                    "upstream SSE frame was invalid",
                ));
                return;
            }
        };
        for frame in frames {
            if let Err(error) = self.push_frame(frame) {
                self.set_pending_error(error);
                break;
            }
        }
    }

    fn process_eof(&mut self) {
        self.upstream_done = true;
        match self.decoder.finish() {
            Ok(Some(frame)) => {
                if let Err(error) = self.push_frame(frame) {
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

    fn push_frame(&mut self, frame: SseFrame) -> Result<(), PendingStreamError> {
        let event = self
            .adapter
            .decode_upstream_event(frame)
            .map_err(|_| PendingStreamError::invalid_response("upstream SSE event was invalid"))?;
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
        if let Some(hard_id) = hard_id {
            self.hard_affinity.bind(&hard_id).map_err(|_| {
                PendingStreamError::local("upstream SSE identity could not be bound")
            })?;
        }
        self.pending.push_back(frame.0);
        Ok(())
    }

    fn finish(&mut self, outcome: StreamOutcome) {
        if self.state == CommitState::Finished {
            return;
        }
        self.state = CommitState::Finished;
        self.cancellation.cancel();
        self.health.take();
        if let Some(mut recorder) = self.attempt_recorder.take() {
            match outcome {
                StreamOutcome::Success => recorder.success(self.status_code),
                StreamOutcome::Error(error_class) => {
                    recorder.stream_error(error_class, self.status_code);
                }
                StreamOutcome::Cancelled => recorder.cancelled(Some(self.status_code)),
            }
        }
        if self.owns_request_completion {
            let error_class = match outcome {
                StreamOutcome::Success => None,
                StreamOutcome::Error(error_class) => Some(error_class),
                StreamOutcome::Cancelled => Some(ErrorClass::Cancelled),
            };
            self.request_recorder.finish(self.status_code, error_class);
        }
        self.permit.take();
    }

    fn release_guards(&mut self) {
        self.state = CommitState::Finished;
        self.cancellation.cancel();
        self.health.take();
        self.permit.take();
    }

    fn finish_precommit_failure(&mut self) -> PublicError {
        let pending = self.pending_error.take();
        let kind = pending
            .as_ref()
            .map_or(PendingStreamErrorKind::InvalidResponse, |error| error.kind);
        match kind {
            PendingStreamErrorKind::Transport {
                retry_safety,
                failure_scope,
            } => {
                if let Some(health) = self.health.take() {
                    health.transport_failure(failure_scope);
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.transport_error(retry_safety, transport_error_class(failure_scope));
                }
            }
            PendingStreamErrorKind::InvalidResponse => {
                if let Some(health) = self.health.take() {
                    health.transport_failure(TransportFailureScope::Endpoint);
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.invalid_response(Some(self.status_code));
                }
            }
            PendingStreamErrorKind::Local => {
                if let Some(health) = self.health.take() {
                    health.success();
                }
                if let Some(mut recorder) = self.attempt_recorder.take() {
                    recorder.local_error(Some(self.status_code), ErrorClass::Internal);
                }
            }
        }
        self.release_guards();
        match kind {
            PendingStreamErrorKind::Local => public_error(
                PublicErrorCode::InternalError,
                "internal stream processing failed",
            ),
            PendingStreamErrorKind::Transport { .. } | PendingStreamErrorKind::InvalidResponse => {
                public_error(
                    PublicErrorCode::UpstreamError,
                    if pending.is_some() {
                        "upstream stream failed before the first event"
                    } else {
                        "upstream stream ended before the first event"
                    },
                )
            }
        }
    }

    fn set_pending_error(&mut self, error: PendingStreamError) {
        if self.pending_error.is_none() {
            self.pending_error = Some(error);
        }
    }

    fn set_transport_error(&mut self, error: &any2api_transport::api::TransportError) {
        self.set_pending_error(PendingStreamError::transport(error));
    }

    fn set_timeout_error(&mut self) {
        self.set_pending_error(PendingStreamError::timeout());
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> CommitState {
        self.state
    }

    #[cfg(test)]
    pub(super) fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }
}

impl Stream for GuardedBody {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(bytes) = this.pending.pop_front() {
                this.state = CommitState::TransportCommitted;
                return Poll::Ready(Some(Ok(bytes)));
            }
            if let Some(error) = this.pending_error.take() {
                this.finish(StreamOutcome::Error(error.kind.error_class()));
                return Poll::Ready(Some(Err(error.error)));
            }
            if this.upstream_done || this.cancellation.is_cancelled() {
                this.finish(StreamOutcome::Success);
                return Poll::Ready(None);
            }
            match this.upstream.as_mut().poll_next(context) {
                Poll::Ready(Some(Ok(chunk))) => this.process_chunk(chunk),
                Poll::Ready(Some(Err(error))) => {
                    this.set_transport_error(&error);
                }
                Poll::Ready(None) => this.process_eof(),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Drop for GuardedBody {
    fn drop(&mut self) {
        self.finish(StreamOutcome::Cancelled);
    }
}

#[derive(Clone, Copy)]
enum StreamOutcome {
    Success,
    Error(ErrorClass),
    Cancelled,
}
