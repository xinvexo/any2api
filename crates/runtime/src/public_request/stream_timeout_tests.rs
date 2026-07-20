use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::{PublicErrorCode, SettingsConfiguration};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{Stream, StreamExt, stream};

use super::{
    stream::PrecommitBudget,
    stream_tests::{
        generation_permit, guarded_body_with_budget, guarded_body_with_budget_health_and_idle,
        guarded_body_with_idle_timeout,
    },
};
use crate::{
    health::{AttemptHealth, EndpointHealthRuntime, ReliabilityPolicy},
    scheduler_epoch::SchedulerEpoch,
};

#[tokio::test(start_paused = true)]
async fn configured_precommit_duration_bounds_the_first_event_wait() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::pending());
    let result = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_millis(25)),
    )
    .prime()
    .await;

    let error = match result {
        Ok(_) => panic!("precommit wait must be bounded"),
        Err(error) => error,
    };
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test]
async fn precommit_deadline_is_checked_after_a_non_cooperative_upstream_poll() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::once(async {
        std::thread::sleep(Duration::from_millis(20));
        Ok(Bytes::from_static(b"data: {\"model\":\"upstream\"}\n\n"))
    }));
    let result = guarded_body_with_budget(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_millis(1)),
    )
    .prime()
    .await;

    let error = match result {
        Ok(_) => panic!("event completed after the deadline must fail"),
        Err(error) => error,
    };
    assert_eq!(error.code, PublicErrorCode::UpstreamError);
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn postcommit_idle_timeout_releases_the_permit_once() {
    let (binding, permit) = generation_permit();
    let upstream_dropped = Arc::new(AtomicBool::new(false));
    let upstream: BoxByteStream = Box::pin(DropObservedStream::new(Arc::clone(&upstream_dropped)));
    let mut body = guarded_body_with_idle_timeout(upstream, permit, Duration::from_millis(25))
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert!(body.next().await.expect("first frame").is_ok());
    assert_eq!(binding.capacity().in_flight(), 1);
    let error = body
        .next()
        .await
        .expect("idle timeout body error")
        .expect_err("idle stream must fail");
    assert!(error.to_string().contains("idle after commit"));
    assert_eq!(binding.capacity().in_flight(), 0);
    assert!(upstream_dropped.load(Ordering::Acquire));
    assert!(body.next().await.is_none());
    drop(body);
    assert_eq!(binding.capacity().in_flight(), 0);
}

struct DropObservedStream {
    first: Option<Bytes>,
    dropped: Arc<AtomicBool>,
}

impl DropObservedStream {
    fn new(dropped: Arc<AtomicBool>) -> Self {
        Self {
            first: Some(Bytes::from_static(b"data: {\"model\":\"upstream\"}\n\n")),
            dropped,
        }
    }
}

impl Stream for DropObservedStream {
    type Item = Result<Bytes, any2api_transport::api::TransportError>;

    fn poll_next(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut().first.take() {
            Some(first) => Poll::Ready(Some(Ok(first))),
            None => Poll::Pending,
        }
    }
}

impl Drop for DropObservedStream {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[tokio::test(start_paused = true)]
async fn postcommit_idle_timer_starts_with_the_first_downstream_frame() {
    let (_binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b"data: {\"model\":\"upstream\"}\n\n",
        ))])
        .chain(stream::pending()),
    );
    let guarded = guarded_body_with_idle_timeout(upstream, permit, Duration::from_millis(25))
        .prime()
        .await
        .expect("primed stream");

    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut body = guarded.into_stream();
    assert!(body.next().await.expect("first frame").is_ok());
    let delivered_at = tokio::time::Instant::now();
    assert!(body.next().await.expect("idle timeout").is_err());
    assert!(delivered_at.elapsed() >= Duration::from_millis(25));
}

#[tokio::test(start_paused = true)]
async fn successful_upstream_chunk_resets_the_postcommit_idle_timer() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b"data: {\"model\":\"upstream\",\"index\":1}\n\n",
        ))])
        .chain(stream::once(async {
            tokio::time::sleep(Duration::from_millis(40)).await;
            Ok(Bytes::from_static(b"data: "))
        }))
        .chain(stream::once(async {
            tokio::time::sleep(Duration::from_millis(40)).await;
            Ok(Bytes::from_static(
                b"{\"model\":\"upstream\",\"index\":2}\n\n",
            ))
        }))
        .chain(stream::pending()),
    );
    let mut body = guarded_body_with_idle_timeout(upstream, permit, Duration::from_millis(50))
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert!(body.next().await.expect("first frame").is_ok());
    let second = body
        .next()
        .await
        .expect("second frame")
        .expect("second frame bytes");
    assert!(String::from_utf8_lossy(&second).contains(r#""index":2"#));
    let reset_at = tokio::time::Instant::now();
    assert!(body.next().await.expect("idle timeout").is_err());
    assert!(reset_at.elapsed() >= Duration::from_millis(50));
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn postcommit_idle_timeout_does_not_penalize_endpoint_health() {
    let (binding, permit) = generation_permit();
    let epoch = SchedulerEpoch::new();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    let mut policy =
        ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability());
    policy.endpoint_failure_threshold = 1;
    let health = AttemptHealth::new(
        binding.generation().clone(),
        "upstream".into(),
        Some(endpoint.try_acquire(&policy).expect("endpoint permit")),
        None,
        policy,
    );
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b"data: {\"model\":\"upstream\"}\n\n",
        ))])
        .chain(stream::pending()),
    );
    let mut body = guarded_body_with_budget_health_and_idle(
        upstream,
        permit,
        PrecommitBudget::new(256 * 1024, Duration::from_secs(5)),
        Some(health),
        Duration::from_millis(25),
    )
    .prime()
    .await
    .expect("primed stream")
    .into_stream();

    assert!(body.next().await.expect("first frame").is_ok());
    assert!(body.next().await.expect("idle timeout").is_err());
    assert_eq!(endpoint.availability(&policy), Ok(()));
    assert_eq!(binding.capacity().in_flight(), 0);
}

#[tokio::test(start_paused = true)]
async fn buffered_frames_do_not_reset_the_postcommit_idle_timer() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(
        stream::iter([Ok(Bytes::from_static(
            b"data: {\"model\":\"upstream\",\"index\":1}\n\ndata: {\"model\":\"upstream\",\"index\":2}\n\ndata: {\"model\":\"upstream\",\"index\":3}\n\n",
        ))])
        .chain(stream::pending()),
    );
    let mut body = guarded_body_with_idle_timeout(upstream, permit, Duration::from_millis(50))
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert!(body.next().await.expect("first frame").is_ok());
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(body.next().await.expect("second buffered frame").is_ok());
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(body.next().await.expect("third buffered frame").is_ok());

    let already_expired_at = tokio::time::Instant::now();
    assert!(body.next().await.expect("idle timeout").is_err());
    assert!(already_expired_at.elapsed() < Duration::from_millis(1));
    assert_eq!(binding.capacity().in_flight(), 0);
}
