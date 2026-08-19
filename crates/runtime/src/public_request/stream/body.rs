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

use any2api_domain::{
    ErrorClass, ProtocolOperation, PublicError, RequestSpeedTier, TokenUsage, UpstreamError,
};
use any2api_protocol::api::{
    BridgeContinuationState, ProtocolContinuationState, ProtocolExchange, ProtocolRetryDelayBasis,
    SseDecoder, StreamTermination,
};
use any2api_provider::api::ProviderDriver;
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::Stream;
use http::HeaderMap;
use tokio::time::Sleep;

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

pub(super) enum PrecommitContinuation {
    Pending,
    Ready(Option<ProtocolContinuationState>),
}

pub(in crate::public_request) struct GuardedBodyParts {
    pub(in crate::public_request) permit: RequestPermit,
    pub(in crate::public_request) health: Option<AttemptHealth>,
    pub(in crate::public_request) continuation_binding: ContinuationBindingCommitter,
    pub(in crate::public_request) attempt_recorder: AttemptRecorder,
    pub(in crate::public_request) quota_activity: Option<OAuthQuotaActivityGuard>,
    pub(in crate::public_request) status_code: u16,
    pub(in crate::public_request) driver: Arc<dyn ProviderDriver>,
    pub(in crate::public_request) upstream_operation: ProtocolOperation,
    pub(in crate::public_request) upstream_headers: HeaderMap,
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
    pub(super) precommit_upstream_failure: Option<PrecommitUpstreamFailure>,
    pub(super) precommit_commit_ready: bool,
    pub(super) permit: Option<RequestPermit>,
    pub(super) health: Option<AttemptHealth>,
    pub(super) continuation_binding: ContinuationBindingCommitter,
    pub(super) continuation_lease: Option<ContinuationLease>,
    pub(super) precommit_continuation: Option<PrecommitContinuation>,
    pub(super) continuation_id: Option<String>,
    pub(super) cancellation: CancellationToken,
    pub(super) state: CommitState,
    pub(super) upstream_done: bool,
    pub(super) decoder_finished: bool,
    pub(super) terminal_seen: bool,
    pub(super) pending_termination: StreamTermination,
    pub(super) pending_upstream_error: Option<UpstreamError>,
    pub(super) attempt_recorder: Option<AttemptRecorder>,
    pub(super) quota_activity: Option<OAuthQuotaActivityGuard>,
    pub(super) request_recorder: RequestRecorder,
    pub(super) status_code: u16,
    pub(super) driver: Arc<dyn ProviderDriver>,
    pub(super) upstream_operation: ProtocolOperation,
    pub(super) upstream_headers: HeaderMap,
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
    pub(super) effective_speed_tier: Option<RequestSpeedTier>,
    pub(super) continuation_id: Option<String>,
    pub(super) continuation_state: BridgeContinuationState,
    pub(super) upstream_error: Option<UpstreamError>,
}

#[derive(Debug)]
pub(in crate::public_request) struct PrecommitUpstreamFailure {
    pub(in crate::public_request) error: UpstreamError,
    pub(in crate::public_request) frame: Bytes,
    pub(in crate::public_request) retry_delay_basis: ProtocolRetryDelayBasis,
}

#[derive(Debug)]
pub(in crate::public_request) enum StreamPrimeFailure {
    Upstream(PrecommitUpstreamFailure),
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
            driver,
            upstream_operation,
            upstream_headers,
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
            precommit_upstream_failure: None,
            precommit_commit_ready: false,
            permit: Some(permit),
            health,
            continuation_binding,
            continuation_lease: None,
            precommit_continuation: None,
            continuation_id: None,
            cancellation: CancellationToken::default(),
            state: CommitState::Pending,
            upstream_done: false,
            decoder_finished: false,
            terminal_seen: false,
            pending_termination: StreamTermination::None,
            pending_upstream_error: None,
            attempt_recorder: Some(attempt_recorder),
            quota_activity,
            request_recorder,
            status_code,
            driver,
            upstream_operation,
            upstream_headers,
            owns_request_completion: false,
            precommit_budget,
            precommit_deadline: None,
            postcommit_idle_timeout,
            idle_timer: None,
        }
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
                this.finish_pending_termination();
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
                    this.pending_upstream_error = frame.upstream_error;
                    this.idle_timer = None;
                    this.finish_pending_termination();
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
    fn finish_pending_termination(&mut self) {
        match std::mem::take(&mut self.pending_termination) {
            StreamTermination::None => unreachable!("terminal state was checked"),
            StreamTermination::Completed => self.finish(StreamOutcome::Success),
            StreamTermination::Failed => {
                if let Some(error) = self.pending_upstream_error.take() {
                    self.finish(StreamOutcome::UpstreamFailure(error));
                } else {
                    self.finish(StreamOutcome::Error {
                        class: ErrorClass::Upstream,
                        message: "upstream response stream reported a failure event".to_owned(),
                    });
                }
            }
        }
    }

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
    UpstreamFailure(UpstreamError),
    Error { class: ErrorClass, message: String },
    Cancelled,
}
