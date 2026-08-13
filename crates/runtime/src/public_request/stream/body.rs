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

use any2api_domain::{ErrorClass, PublicError, TokenUsage};
use any2api_protocol::api::{
    BridgeContinuationState, ProtocolExchange, SseDecoder, StreamRejection, StreamTermination,
};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use tokio::time::{Sleep, timeout};

use super::super::{PublicResponseStream, RequestPermit};
use super::{PrecommitBudget, pending_failure::PendingStreamError};
use crate::request_telemetry::{AttemptRecorder, RequestRecorder};
use crate::{
    affinity::{ContinuationBindingCommitter, ContinuationLease},
    health::AttemptHealth,
    oauth::OAuthQuotaActivityGuard,
};

#[derive(Clone, Debug, Default)]
pub(super) struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub(super) fn cancel(&self) {
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

pub(in crate::public_request) struct GuardedBodyParts {
    pub(in crate::public_request) permit: RequestPermit,
    pub(in crate::public_request) health: Option<AttemptHealth>,
    pub(in crate::public_request) continuation_binding: ContinuationBindingCommitter,
    pub(in crate::public_request) attempt_recorder: AttemptRecorder,
    pub(in crate::public_request) quota_activity: Option<OAuthQuotaActivityGuard>,
    pub(in crate::public_request) status_code: u16,
    pub(in crate::public_request) precommit_budget: PrecommitBudget,
    pub(in crate::public_request) postcommit_idle_timeout: Duration,
}

pub(in crate::public_request) struct GuardedBody {
    pub(super) upstream: BoxByteStream,
    pub(super) decoder: SseDecoder,
    pub(super) exchange: ProtocolExchange,
    pub(super) public_model: String,
    pub(super) buffered_chunk: Option<Bytes>,
    pub(super) pending: VecDeque<PendingFrame>,
    pub(super) pending_error: Option<PendingStreamError>,
    pub(super) precommit_retry: Option<StreamRejection>,
    pub(super) precommit_commit_ready: bool,
    pub(super) permit: Option<RequestPermit>,
    pub(super) health: Option<AttemptHealth>,
    pub(super) continuation_binding: ContinuationBindingCommitter,
    pub(super) continuation_lease: Option<ContinuationLease>,
    pub(super) continuation_id: Option<String>,
    pub(super) cancellation: CancellationToken,
    pub(super) state: CommitState,
    pub(super) upstream_done: bool,
    pub(super) decoder_finished: bool,
    pub(super) terminal_seen: bool,
    pub(super) pending_termination: StreamTermination,
    pub(super) attempt_recorder: Option<AttemptRecorder>,
    pub(super) quota_activity: Option<OAuthQuotaActivityGuard>,
    pub(super) request_recorder: RequestRecorder,
    pub(super) status_code: u16,
    pub(super) owns_request_completion: bool,
    pub(super) precommit_budget: PrecommitBudget,
    pub(super) precommit_deadline: Option<Instant>,
    pub(super) postcommit_idle_timeout: Duration,
    pub(super) idle_timer: Option<Pin<Box<Sleep>>>,
}

pub(super) struct PendingFrame {
    pub(super) bytes: Bytes,
    pub(super) has_content_delta: bool,
    pub(super) termination: StreamTermination,
    pub(super) token_usage: TokenUsage,
    pub(super) continuation_id: Option<String>,
    pub(super) continuation_state: BridgeContinuationState,
}

#[derive(Debug)]
pub(in crate::public_request) struct PrecommitStreamRejection {
    pub(in crate::public_request) rejection: StreamRejection,
    pub(in crate::public_request) frame: Bytes,
}

#[derive(Debug)]
pub(in crate::public_request) enum StreamPrimeFailure {
    Retryable(PrecommitStreamRejection),
    Public(PublicError),
}

impl GuardedBody {
    pub(in crate::public_request) fn new(
        upstream: BoxByteStream,
        exchange: ProtocolExchange,
        public_model: impl Into<String>,
        parts: GuardedBodyParts,
    ) -> Self {
        let GuardedBodyParts {
            permit,
            health,
            continuation_binding,
            attempt_recorder,
            quota_activity,
            status_code,
            precommit_budget,
            postcommit_idle_timeout,
        } = parts;
        let request_recorder = attempt_recorder.request();
        let decoder = SseDecoder::new(precommit_budget.max_frame_bytes());
        Self {
            upstream,
            decoder,
            exchange,
            public_model: public_model.into(),
            buffered_chunk: None,
            pending: VecDeque::new(),
            pending_error: None,
            precommit_retry: None,
            precommit_commit_ready: false,
            permit: Some(permit),
            health,
            continuation_binding,
            continuation_lease: None,
            continuation_id: None,
            cancellation: CancellationToken::default(),
            state: CommitState::Pending,
            upstream_done: false,
            decoder_finished: false,
            terminal_seen: false,
            pending_termination: StreamTermination::None,
            attempt_recorder: Some(attempt_recorder),
            quota_activity,
            request_recorder,
            status_code,
            owns_request_completion: false,
            precommit_budget,
            precommit_deadline: None,
            postcommit_idle_timeout,
            idle_timer: None,
        }
    }

    #[cfg(test)]
    pub(super) async fn prime(self) -> Result<Self, PublicError> {
        self.prime_attempt()
            .await
            .map_err(StreamPrimeFailure::into_public)
    }

    pub(in crate::public_request) async fn prime_attempt(
        mut self,
    ) -> Result<Self, StreamPrimeFailure> {
        let deadline = Instant::now() + self.precommit_budget.max_duration();
        self.precommit_deadline = Some(deadline);
        loop {
            if self.precommit_commit_ready
                || self.precommit_retry.is_some()
                || self.pending_error.is_some()
            {
                break;
            }
            if self.process_buffered_frame(Some(deadline)) {
                continue;
            }
            if self.upstream_done {
                self.finish_decoder(Some(deadline));
                if self.precommit_commit_ready
                    || self.precommit_retry.is_some()
                    || self.pending_error.is_some()
                {
                    continue;
                }
                break;
            }
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
        if let Some(rejection) = self.precommit_retry.take() {
            let frame = self
                .pending
                .back()
                .map(|frame| frame.bytes.clone())
                .expect("precommit rejection has an encoded terminal frame");
            self.finish_precommit_rejection(rejection);
            return Err(StreamPrimeFailure::Retryable(PrecommitStreamRejection {
                rejection,
                frame,
            }));
        }
        if self.pending_error.is_some() && !self.precommit_commit_ready {
            return Err(StreamPrimeFailure::Public(self.finish_precommit_failure()));
        }
        if self.pending.is_empty() {
            return Err(StreamPrimeFailure::Public(self.finish_precommit_failure()));
        }
        if let Err(error) = self.commit_precommit_frames(Some(deadline)) {
            self.set_pending_error(error);
            return Err(StreamPrimeFailure::Public(self.finish_precommit_failure()));
        }
        Ok(self)
    }

    pub(in crate::public_request) fn fail_before_handoff(
        &mut self,
        error_class: ErrorClass,
        message: impl AsRef<str>,
    ) {
        if let Some(mut recorder) = self.attempt_recorder.take() {
            recorder.local_error(Some(self.status_code), error_class, message);
        }
        self.release_guards();
    }

    pub(in crate::public_request) fn into_stream(mut self) -> PublicResponseStream {
        self.owns_request_completion = true;
        Box::pin(self)
    }

    pub(in crate::public_request) fn precommit_deadline(&self) -> Instant {
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

    #[cfg(test)]
    pub(super) fn stream_timing(&self) -> any2api_domain::RequestAttemptStreamTiming {
        self.attempt_recorder
            .as_ref()
            .expect("attempt recorder remains active")
            .stream_timing()
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
            if this.pending_termination.is_terminal() {
                match std::mem::take(&mut this.pending_termination) {
                    StreamTermination::None => unreachable!("terminal state was checked"),
                    StreamTermination::Completed => this.finish(StreamOutcome::Success),
                    StreamTermination::Failed => this.finish(StreamOutcome::Error {
                        class: ErrorClass::Upstream,
                        message: "upstream response stream reported a failure event".to_owned(),
                    }),
                }
                return Poll::Ready(None);
            }
            if let Some(frame) = this.pending.pop_front() {
                this.state = CommitState::TransportCommitted;
                if !frame.bytes.is_empty()
                    && let Some(recorder) = this.attempt_recorder.as_mut()
                {
                    recorder.observe_first_downstream_byte();
                }
                if frame.has_content_delta {
                    this.request_recorder.observe_first_token();
                }
                if frame.termination.is_terminal() {
                    this.upstream = Box::pin(futures_util::stream::empty());
                    this.pending_termination = frame.termination;
                    this.idle_timer = None;
                } else {
                    this.start_idle_timer();
                }
                return Poll::Ready(Some(Ok(frame.bytes)));
            }
            if let Some(error) = this.pending_error.take() {
                this.finish(StreamOutcome::Error {
                    class: error.kind.error_class(),
                    message: error.message(),
                });
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
        let deadline = tokio::time::Instant::now() + self.postcommit_idle_timeout;
        if let Some(timer) = self.idle_timer.as_mut() {
            timer.as_mut().reset(deadline);
        } else {
            self.idle_timer = Some(Box::pin(tokio::time::sleep_until(deadline)));
        }
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

pub(super) enum StreamOutcome {
    Success,
    Error { class: ErrorClass, message: String },
    Cancelled,
}

#[cfg(test)]
impl StreamPrimeFailure {
    fn into_public(self) -> PublicError {
        match self {
            Self::Public(error) => error,
            Self::Retryable(rejected) => super::super::response::public_error(
                any2api_domain::PublicErrorCode::UpstreamError,
                rejected.rejection.code(),
            ),
        }
    }
}
