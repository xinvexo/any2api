use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::RetrySafety;
use bytes::Bytes;
use futures_core::Stream;
use tokio::time::{Instant, Sleep, sleep};

use crate::{
    api::BoxByteStream,
    error::{TransportError, TransportErrorStage, TransportFailureScope},
};

/// Applies the request read timeout between body chunks so a stalled upstream
/// stream surfaces as an error instead of hanging its consumer forever.
pub(super) fn timeout_body(
    body: BoxByteStream,
    idle_timeout: Duration,
    failure_scope: TransportFailureScope,
) -> BoxByteStream {
    Box::pin(IdleTimeoutBody {
        inner: body,
        idle_timeout,
        deadline: Box::pin(sleep(idle_timeout)),
        failure_scope,
        timed_out: false,
    })
}

struct IdleTimeoutBody {
    inner: BoxByteStream,
    idle_timeout: Duration,
    deadline: Pin<Box<Sleep>>,
    failure_scope: TransportFailureScope,
    timed_out: bool,
}

impl Stream for IdleTimeoutBody {
    type Item = Result<Bytes, TransportError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.timed_out {
            return Poll::Ready(None);
        }
        match this.inner.as_mut().poll_next(context) {
            Poll::Ready(item) => {
                this.deadline
                    .as_mut()
                    .reset(Instant::now() + this.idle_timeout);
                Poll::Ready(item)
            }
            Poll::Pending => {
                if this.deadline.as_mut().poll(context).is_ready() {
                    this.timed_out = true;
                    return Poll::Ready(Some(Err(TransportError::new(
                        TransportErrorStage::ReadBody,
                        this.failure_scope,
                        RetrySafety::Ambiguous,
                        "upstream response body read timed out",
                    ))));
                }
                Poll::Pending
            }
        }
    }
}
