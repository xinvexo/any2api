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

use any2api_domain::{PublicError, PublicErrorCode};
use any2api_protocol::{
    SseDecoder,
    api::{ProtocolAdapter, SseFrame},
};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use tokio::time::timeout;

use super::{PublicResponseStream, RequestPermit, response::public_error};
use crate::affinity::HardAffinityCommitter;

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

pub(super) struct GuardedBody {
    upstream: BoxByteStream,
    decoder: SseDecoder,
    adapter: Arc<dyn ProtocolAdapter>,
    public_model: String,
    pending: VecDeque<Bytes>,
    pending_error: Option<io::Error>,
    permit: Option<RequestPermit>,
    hard_affinity: HardAffinityCommitter,
    cancellation: CancellationToken,
    state: CommitState,
    upstream_done: bool,
}

impl GuardedBody {
    pub(super) fn new(
        upstream: BoxByteStream,
        adapter: Arc<dyn ProtocolAdapter>,
        public_model: impl Into<String>,
        permit: RequestPermit,
        hard_affinity: HardAffinityCommitter,
    ) -> Self {
        Self {
            upstream,
            decoder: SseDecoder::default(),
            adapter,
            public_model: public_model.into(),
            pending: VecDeque::new(),
            pending_error: None,
            permit: Some(permit),
            hard_affinity,
            cancellation: CancellationToken::default(),
            state: CommitState::Pending,
            upstream_done: false,
        }
    }

    pub(super) async fn prime(mut self) -> Result<PublicResponseStream, PublicError> {
        let deadline = Instant::now() + PRECOMMIT_TIMEOUT;
        while self.pending.is_empty() && self.pending_error.is_none() && !self.upstream_done {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.pending_error = Some(stream_error("upstream stream precommit timed out"));
                break;
            }
            match timeout(remaining, self.upstream.next()).await {
                Ok(Some(Ok(chunk))) => self.process_chunk(chunk),
                Ok(Some(Err(_))) => {
                    self.pending_error = Some(stream_error("upstream stream failed"));
                }
                Ok(None) => self.process_eof(),
                Err(_) => {
                    self.pending_error = Some(stream_error("upstream stream precommit timed out"));
                }
            }
        }
        if self.pending.is_empty() {
            self.finish();
            return Err(public_error(
                PublicErrorCode::UpstreamError,
                if self.pending_error.is_some() {
                    "upstream stream failed before the first event"
                } else {
                    "upstream stream ended before the first event"
                },
            ));
        }
        Ok(Box::pin(self))
    }

    fn process_chunk(&mut self, chunk: Bytes) {
        let frames = match self.decoder.push(&chunk) {
            Ok(frames) => frames,
            Err(_) => {
                self.pending_error = Some(stream_error("upstream SSE frame was invalid"));
                return;
            }
        };
        for frame in frames {
            if let Err(error) = self.push_frame(frame) {
                self.pending_error = Some(error);
                break;
            }
        }
    }

    fn process_eof(&mut self) {
        self.upstream_done = true;
        match self.decoder.finish() {
            Ok(Some(frame)) => {
                if let Err(error) = self.push_frame(frame) {
                    self.pending_error = Some(error);
                }
            }
            Ok(None) => {}
            Err(_) => {
                self.pending_error = Some(stream_error("upstream SSE frame was invalid"));
            }
        }
    }

    fn push_frame(&mut self, frame: SseFrame) -> Result<(), io::Error> {
        let event = self
            .adapter
            .decode_upstream_event(frame)
            .map_err(|_| stream_error("upstream SSE event was invalid"))?;
        let hard_id = self
            .adapter
            .hard_affinity_id_from_event(self.hard_affinity.operation(), &event)
            .map_err(|_| stream_error("upstream SSE identity was invalid"))?;
        let frame = self
            .adapter
            .encode_egress_event(event, &self.public_model)
            .map_err(|_| stream_error("upstream SSE event could not be encoded"))?;
        if let Some(hard_id) = hard_id {
            self.hard_affinity
                .bind(&hard_id)
                .map_err(|_| stream_error("upstream SSE identity could not be bound"))?;
        }
        self.pending.push_back(frame.0);
        Ok(())
    }

    fn finish(&mut self) {
        if self.state == CommitState::Finished {
            return;
        }
        self.state = CommitState::Finished;
        self.cancellation.cancel();
        self.permit.take();
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
                this.finish();
                return Poll::Ready(Some(Err(error)));
            }
            if this.upstream_done || this.cancellation.is_cancelled() {
                this.finish();
                return Poll::Ready(None);
            }
            match this.upstream.as_mut().poll_next(context) {
                Poll::Ready(Some(Ok(chunk))) => this.process_chunk(chunk),
                Poll::Ready(Some(Err(_))) => {
                    this.pending_error = Some(stream_error("upstream stream failed"));
                }
                Poll::Ready(None) => this.process_eof(),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

impl Drop for GuardedBody {
    fn drop(&mut self) {
        self.finish();
    }
}

fn stream_error(message: &'static str) -> io::Error {
    io::Error::other(message)
}
