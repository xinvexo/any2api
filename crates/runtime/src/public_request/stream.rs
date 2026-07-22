mod completion;
mod frame_pipeline;
mod pending_failure;
mod precommit_budget;

use std::{
    collections::VecDeque,
    future::Future,
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use any2api_domain::{ErrorClass, PublicError};
use any2api_protocol::{SseDecoder, api::ProtocolAdapter};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use tokio::time::{Sleep, timeout};

use self::pending_failure::PendingStreamError;
pub(super) use self::precommit_budget::PrecommitBudget;
use super::{PublicResponseStream, RequestPermit};
use crate::request_telemetry::{AttemptRecorder, RequestRecorder};
use crate::{affinity::HardAffinityCommitter, health::AttemptHealth};

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
    pub(super) precommit_budget: PrecommitBudget,
    pub(super) postcommit_idle_timeout: Duration,
}

pub(super) struct GuardedBody {
    upstream: BoxByteStream,
    decoder: SseDecoder,
    adapter: Arc<dyn ProtocolAdapter>,
    public_model: String,
    buffered_chunk: Option<Bytes>,
    pending: VecDeque<PendingFrame>,
    pending_error: Option<PendingStreamError>,
    permit: Option<RequestPermit>,
    health: Option<AttemptHealth>,
    hard_affinity: HardAffinityCommitter,
    cancellation: CancellationToken,
    state: CommitState,
    upstream_done: bool,
    decoder_finished: bool,
    attempt_recorder: Option<AttemptRecorder>,
    request_recorder: RequestRecorder,
    status_code: u16,
    owns_request_completion: bool,
    precommit_budget: PrecommitBudget,
    precommit_deadline: Option<Instant>,
    postcommit_idle_timeout: Duration,
    idle_timer: Option<Pin<Box<Sleep>>>,
}

pub(super) struct PendingFrame {
    bytes: Bytes,
    has_content_delta: bool,
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
            precommit_budget,
            postcommit_idle_timeout,
        } = parts;
        let request_recorder = attempt_recorder.request();
        let decoder = SseDecoder::new(precommit_budget.max_frame_bytes());
        Self {
            upstream,
            decoder,
            adapter,
            public_model: public_model.into(),
            buffered_chunk: None,
            pending: VecDeque::new(),
            pending_error: None,
            permit: Some(permit),
            health,
            hard_affinity,
            cancellation: CancellationToken::default(),
            state: CommitState::Pending,
            upstream_done: false,
            decoder_finished: false,
            attempt_recorder: Some(attempt_recorder),
            request_recorder,
            status_code,
            owns_request_completion: false,
            precommit_budget,
            precommit_deadline: None,
            postcommit_idle_timeout,
            idle_timer: None,
        }
    }

    pub(super) async fn prime(mut self) -> Result<Self, PublicError> {
        let deadline = Instant::now() + self.precommit_budget.max_duration();
        self.precommit_deadline = Some(deadline);
        while self.pending.is_empty() && self.pending_error.is_none() && !self.upstream_done {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.set_timeout_error();
                break;
            }
            match timeout(remaining, self.upstream.next()).await {
                Ok(Some(Ok(chunk))) => self.process_chunk(chunk, Some(deadline)),
                Ok(Some(Err(error))) => {
                    self.set_transport_error(&error);
                }
                Ok(None) => self.process_eof(Some(deadline)),
                Err(_) => self.set_timeout_error(),
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

    pub(super) fn precommit_deadline(&self) -> Instant {
        self.precommit_deadline.expect("stream was primed")
    }

    #[cfg(test)]
    pub(super) fn state(&self) -> CommitState {
        self.state
    }

    #[cfg(test)]
    pub(super) fn cancellation(&self) -> CancellationToken {
        self.cancellation.clone()
    }

    #[cfg(test)]
    pub(super) fn pending_frame_count(&self) -> usize {
        self.pending.len()
    }
}

impl Stream for GuardedBody {
    type Item = Result<Bytes, io::Error>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.state == CommitState::Finished {
                return Poll::Ready(None);
            }
            if let Some(frame) = this.pending.pop_front() {
                this.state = CommitState::TransportCommitted;
                this.start_idle_timer();
                if frame.has_content_delta {
                    this.request_recorder.observe_first_token();
                }
                return Poll::Ready(Some(Ok(frame.bytes)));
            }
            if let Some(error) = this.pending_error.take() {
                this.finish(StreamOutcome::Error(error.kind.error_class()));
                return Poll::Ready(Some(Err(error.error)));
            }
            if this.process_buffered_frame(None) {
                continue;
            }
            if this.upstream_done {
                this.finish_decoder(None);
                if !this.decoder_finished
                    || !this.pending.is_empty()
                    || this.pending_error.is_some()
                {
                    continue;
                }
            }
            if this.upstream_done || this.cancellation.is_cancelled() {
                this.finish(StreamOutcome::Success);
                return Poll::Ready(None);
            }
            match this.upstream.as_mut().poll_next(context) {
                Poll::Ready(Some(Ok(chunk))) => {
                    this.reset_idle_timer();
                    this.process_chunk(chunk, None);
                }
                Poll::Ready(Some(Err(error))) => {
                    this.set_transport_error(&error);
                }
                Poll::Ready(None) => this.process_eof(None),
                Poll::Pending => {
                    if this.idle_timer_elapsed(context) {
                        this.set_postcommit_idle_timeout_error();
                        continue;
                    }
                    return Poll::Pending;
                }
            }
        }
    }
}

impl GuardedBody {
    fn start_idle_timer(&mut self) {
        if self.idle_timer.is_none() {
            self.reset_idle_timer();
        }
    }

    fn reset_idle_timer(&mut self) {
        self.idle_timer = Some(Box::pin(tokio::time::sleep(self.postcommit_idle_timeout)));
    }

    fn idle_timer_elapsed(&mut self, context: &mut Context<'_>) -> bool {
        self.idle_timer
            .as_mut()
            .is_some_and(|timer| timer.as_mut().poll(context).is_ready())
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
