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

    pub(super) fn snapshot(&self) -> HttpBodyCapture {
        HttpBodyCapture {
            content: self.content.clone(),
            total_bytes: self.total_bytes,
            complete: self.complete,
            truncated: self.total_bytes > u64::try_from(self.content.len()).unwrap_or(u64::MAX),
        }
    }
}

#[derive(Clone)]
pub(super) struct SharedBodyCapture(Arc<Mutex<BodyCapture>>);

impl SharedBodyCapture {
    pub(super) fn new(initially_complete: bool) -> Self {
        Self(Arc::new(Mutex::new(BodyCapture::new(initially_complete))))
    }

    fn observe(&self, bytes: &[u8]) {
        self.0.lock().expect("HTTP body capture").observe(bytes);
    }

    fn finish(&self, complete: bool) {
        self.0.lock().expect("HTTP body capture").finish(complete);
    }

    pub(super) fn snapshot(&self) -> HttpBodyCapture {
        self.0.lock().expect("HTTP body capture").snapshot()
    }
}

pub(super) struct RequestCaptureBody {
    inner: Body,
    capture: SharedBodyCapture,
    finished: bool,
}

impl RequestCaptureBody {
    pub(super) fn new(inner: Body, capture: SharedBodyCapture) -> Self {
        let finished = inner.is_end_stream();
        if finished {
            capture.finish(true);
        }
        Self {
            inner,
            capture,
            finished,
        }
    }

    fn finish(&mut self, complete: bool) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.capture.finish(complete);
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

        let snapshot = capture.snapshot();
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
        let capture = SharedBodyCapture::new(false);
        let body = RequestCaptureBody::new(Body::from("raw request"), capture.clone());
        let forwarded = BodyExt::collect(body)
            .await
            .expect("forwarded body")
            .to_bytes();

        assert_eq!(forwarded, "raw request");
        let snapshot = capture.snapshot();
        assert_eq!(snapshot.content, b"raw request");
        assert_eq!(snapshot.total_bytes, 11);
        assert!(snapshot.complete);
        assert!(!snapshot.truncated);
    }

    #[test]
    fn unread_request_body_is_marked_incomplete() {
        let capture = SharedBodyCapture::new(false);
        drop(RequestCaptureBody::new(
            Body::from("never consumed"),
            capture.clone(),
        ));

        let snapshot = capture.snapshot();
        assert!(snapshot.content.is_empty());
        assert!(!snapshot.complete);
        assert!(!snapshot.truncated);
    }
}
