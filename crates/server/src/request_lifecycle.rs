use std::{
    error::Error,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use any2api_runtime::api::{ActiveRequestGuard, ProcessLifecycle};
use axum::{
    body::{Body, Bytes, HttpBody},
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body::{Frame, SizeHint};

pub(crate) async fn track(
    State(lifecycle): State<ProcessLifecycle>,
    request: Request,
    next: Next,
) -> Response {
    let Some(mut guard) = lifecycle.track_request() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let forced = lifecycle.clone();
    let response = tokio::select! {
        biased;
        () = forced.forced() => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        response = next.run(request) => response,
    };
    release_memory_reclamation_blocker(&response, &mut guard);
    response.map(|body| Body::new(TrackedBody::new(body, guard, lifecycle)))
}

#[derive(Clone, Copy)]
struct DoesNotBlockMemoryReclamation;

pub(crate) fn allow_memory_reclamation(response: &mut Response) {
    response
        .extensions_mut()
        .insert(DoesNotBlockMemoryReclamation);
}

fn release_memory_reclamation_blocker(response: &Response, guard: &mut ActiveRequestGuard) {
    if allows_memory_reclamation(response) {
        guard.release_memory_reclamation_blocker();
    }
}

pub(crate) fn allows_memory_reclamation(response: &Response) -> bool {
    response
        .extensions()
        .get::<DoesNotBlockMemoryReclamation>()
        .is_some()
}

struct TrackedBody {
    inner: Body,
    guard: Option<ActiveRequestGuard>,
    forced: Pin<Box<dyn Future<Output = ()> + Send>>,
    finished: bool,
}

impl TrackedBody {
    fn new(inner: Body, guard: ActiveRequestGuard, lifecycle: ProcessLifecycle) -> Self {
        Self {
            inner,
            guard: Some(guard),
            forced: Box::pin(async move { lifecycle.forced().await }),
            finished: false,
        }
    }

    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.inner = Body::empty();
        self.guard.take();
    }
}

impl HttpBody for TrackedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.finished {
            return Poll::Ready(None);
        }
        if self.forced.as_mut().poll(context).is_ready() {
            self.finish();
            return Poll::Ready(Some(Err(axum::Error::new(ShutdownBodyError))));
        }
        let frame = Pin::new(&mut self.inner).poll_frame(context);
        if matches!(frame, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            self.finish();
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.finished || self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        if self.finished {
            SizeHint::with_exact(0)
        } else {
            self.inner.size_hint()
        }
    }
}

#[derive(Debug)]
struct ShutdownBodyError;

impl fmt::Display for ShutdownBodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("response cancelled because the service is shutting down")
    }
}

impl Error for ShutdownBodyError {}

#[cfg(test)]
mod tests {
    use std::{
        future::poll_fn,
        pin::Pin,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll},
    };

    use any2api_runtime::api::ProcessLifecycle;
    use axum::body::{Body, Bytes, HttpBody};
    use http_body::Frame;

    use super::{TrackedBody, allow_memory_reclamation, release_memory_reclamation_blocker};

    #[test]
    fn request_guard_is_held_until_the_response_body_is_dropped() {
        let lifecycle = ProcessLifecycle::new();
        let guard = lifecycle.track_request().expect("request guard");
        let body = TrackedBody::new(Body::from("ok"), guard, lifecycle.clone());

        assert_eq!(lifecycle.active_requests(), 1);
        assert_eq!(lifecycle.memory_reclamation_blockers(), 1);
        drop(body);
        assert_eq!(lifecycle.active_requests(), 0);
        assert_eq!(lifecycle.memory_reclamation_blockers(), 0);
    }

    #[test]
    fn marked_response_releases_only_the_memory_reclamation_blocker() {
        let lifecycle = ProcessLifecycle::new();
        let mut guard = lifecycle.track_request().expect("request guard");
        let mut response = axum::response::Response::new(Body::empty());
        allow_memory_reclamation(&mut response);

        release_memory_reclamation_blocker(&response, &mut guard);
        let body = TrackedBody::new(Body::empty(), guard, lifecycle.clone());

        assert_eq!(lifecycle.active_requests(), 1);
        assert_eq!(lifecycle.memory_reclamation_blockers(), 0);
        drop(body);
        assert_eq!(lifecycle.active_requests(), 0);
    }

    #[tokio::test]
    async fn forced_shutdown_drops_a_silent_inner_body_and_returns_an_error() {
        let lifecycle = ProcessLifecycle::new();
        let dropped = Arc::new(AtomicBool::new(false));
        let inner = Body::new(PendingBody(Arc::clone(&dropped)));
        let guard = lifecycle.track_request().expect("request guard");
        let mut body = Box::pin(TrackedBody::new(inner, guard, lifecycle.clone()));

        lifecycle.force();
        let frame = poll_fn(|context| body.as_mut().poll_frame(context)).await;

        assert!(frame.expect("cancellation frame").is_err());
        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(lifecycle.active_requests(), 0);
        assert_eq!(lifecycle.memory_reclamation_blockers(), 0);
    }

    struct PendingBody(Arc<AtomicBool>);

    impl HttpBody for PendingBody {
        type Data = Bytes;
        type Error = std::convert::Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
            Poll::Pending
        }
    }

    impl Drop for PendingBody {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
}
