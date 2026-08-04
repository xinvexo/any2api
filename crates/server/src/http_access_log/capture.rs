use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use any2api_domain::{HttpBodyCapture, MAX_HTTP_ACCESS_LOG_BODY_CAPTURE_BYTES};
use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};

pub(super) struct BodyCapture {
    content: Vec<u8>,
    total_bytes: u64,
    complete: bool,
    finished: bool,
}

impl BodyCapture {
    pub(super) const fn new(initially_complete: bool) -> Self {
        Self {
            content: Vec::new(),
            total_bytes: 0,
            complete: initially_complete,
            finished: initially_complete,
        }
    }

    pub(super) fn observe(&mut self, bytes: &[u8]) {
        self.total_bytes = self
            .total_bytes
            .saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let remaining = MAX_HTTP_ACCESS_LOG_BODY_CAPTURE_BYTES.saturating_sub(self.content.len());
        self.content
            .extend_from_slice(&bytes[..bytes.len().min(remaining)]);
    }

    pub(super) fn finish(&mut self, complete: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.complete = complete;
    }

    pub(super) fn take_snapshot(&mut self) -> HttpBodyCapture {
        let content = std::mem::take(&mut self.content);
        HttpBodyCapture {
            truncated: self.total_bytes > u64::try_from(content.len()).unwrap_or(u64::MAX),
            content,
            total_bytes: self.total_bytes,
            complete: self.complete,
        }
    }
}

/// Hand-off slot between the request body wrapper and the access-log
/// completion. The wrapper owns its [`BodyCapture`] outright, so streaming
/// chunks never take a lock; the finished snapshot is published here once and
/// taken once when the response completes.
#[derive(Clone)]
pub(super) struct RequestBodyCaptureSlot(Arc<Mutex<Option<HttpBodyCapture>>>);

impl RequestBodyCaptureSlot {
    pub(super) fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    fn publish(&self, snapshot: HttpBodyCapture) {
        *self.0.lock().expect("HTTP request body capture slot") = Some(snapshot);
    }

    pub(super) fn take(&self) -> HttpBodyCapture {
        self.0
            .lock()
            .expect("HTTP request body capture slot")
            .take()
            .unwrap_or(HttpBodyCapture {
                content: Vec::new(),
                total_bytes: 0,
                complete: false,
                truncated: false,
            })
    }
}

pub(super) struct RequestCaptureBody {
    inner: Body,
    capture: BodyCapture,
    slot: RequestBodyCaptureSlot,
    finished: bool,
}

impl RequestCaptureBody {
    pub(super) fn new(inner: Body, slot: RequestBodyCaptureSlot) -> Self {
        let initially_complete = inner.is_end_stream();
        let mut body = Self {
            inner,
            capture: BodyCapture::new(initially_complete),
            slot,
            finished: false,
        };
        if initially_complete {
            body.finish(true);
        }
        body
    }

    fn finish(&mut self, complete: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.capture.finish(complete);
        self.slot.publish(self.capture.take_snapshot());
    }
}

impl HttpBody for RequestCaptureBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = Pin::new(&mut self.inner).poll_frame(context);
        match &frame {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    self.capture.observe(data);
                }
                if self.inner.is_end_stream() {
                    self.finish(true);
                }
            }
            Poll::Ready(Some(Err(_))) => self.finish(false),
            Poll::Ready(None) => self.finish(true),
            Poll::Pending => {}
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.finished || self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for RequestCaptureBody {
    fn drop(&mut self) {
        self.finish(self.inner.is_end_stream());
    }
}

#[cfg(test)]
mod tests {
    use http_body_util::BodyExt;

    use super::*;

    #[test]
    fn capture_retains_prefix_and_reports_observed_total() {
        let mut capture = BodyCapture::new(false);
        capture.observe(&vec![b'a'; MAX_HTTP_ACCESS_LOG_BODY_CAPTURE_BYTES]);
        capture.observe(b"tail");
        capture.finish(true);

        let captured_pointer = capture.content.as_ptr();
        let snapshot = capture.take_snapshot();
        assert_eq!(snapshot.content.as_ptr(), captured_pointer);
        assert!(capture.content.is_empty());
        assert_eq!(
            snapshot.content.len(),
            MAX_HTTP_ACCESS_LOG_BODY_CAPTURE_BYTES
        );
        assert_eq!(
            snapshot.total_bytes,
            u64::try_from(MAX_HTTP_ACCESS_LOG_BODY_CAPTURE_BYTES + 4).expect("body size")
        );
        assert!(snapshot.complete);
        assert!(snapshot.truncated);
    }

    #[tokio::test]
    async fn request_wrapper_forwards_and_captures_body() {
        let slot = RequestBodyCaptureSlot::new();
        let body = RequestCaptureBody::new(Body::from("raw request"), slot.clone());
        let forwarded = BodyExt::collect(body)
            .await
            .expect("forwarded body")
            .to_bytes();

        assert_eq!(forwarded, "raw request");
        let snapshot = slot.take();
        assert_eq!(snapshot.content, b"raw request");
        assert_eq!(snapshot.total_bytes, 11);
        assert!(snapshot.complete);
        assert!(!snapshot.truncated);
    }

    #[test]
    fn unread_request_body_is_marked_incomplete() {
        let slot = RequestBodyCaptureSlot::new();
        drop(RequestCaptureBody::new(
            Body::from("never consumed"),
            slot.clone(),
        ));

        let snapshot = slot.take();
        assert!(snapshot.content.is_empty());
        assert!(!snapshot.complete);
        assert!(!snapshot.truncated);
    }

    #[test]
    fn unpublished_slot_yields_an_incomplete_empty_capture() {
        let slot = RequestBodyCaptureSlot::new();
        let snapshot = slot.take();
        assert!(snapshot.content.is_empty());
        assert_eq!(snapshot.total_bytes, 0);
        assert!(!snapshot.complete);
        assert!(!snapshot.truncated);
    }

    #[test]
    fn empty_request_body_publishes_a_complete_capture_immediately() {
        let slot = RequestBodyCaptureSlot::new();
        let _body = RequestCaptureBody::new(Body::empty(), slot.clone());
        let snapshot = slot.take();
        assert!(snapshot.content.is_empty());
        assert!(snapshot.complete);
        assert!(!snapshot.truncated);
    }
}
