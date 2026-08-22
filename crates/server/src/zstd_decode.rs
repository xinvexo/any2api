use std::{io::ErrorKind, sync::Arc, time::Duration};

use any2api_payload_buffer::{PayloadBuffer, PayloadBufferError};
use any2api_runtime::api::{ProcessLifecycle, STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES};
use axum::body::Bytes;
use tokio::sync::Semaphore;

const MIN_CONCURRENT_ZSTD_DECODES: usize = 2;
const ZSTD_QUEUE_TIMEOUT: Duration = Duration::from_secs(10);
const ZSTD_MAX_WINDOW_LOG: u32 = 25;

fn default_decode_limit() -> usize {
    std::thread::available_parallelism().map_or(MIN_CONCURRENT_ZSTD_DECODES, |parallelism| {
        (parallelism.get() / 2).max(MIN_CONCURRENT_ZSTD_DECODES)
    })
}

#[derive(Clone)]
pub(crate) struct ZstdDecoder {
    lifecycle: ProcessLifecycle,
    permits: Arc<Semaphore>,
    queue_timeout: Duration,
}

impl ZstdDecoder {
    pub(crate) fn new(lifecycle: ProcessLifecycle) -> Self {
        Self::with_limit(lifecycle, default_decode_limit())
    }

    fn with_limit(lifecycle: ProcessLifecycle, limit: usize) -> Self {
        Self {
            lifecycle,
            permits: Arc::new(Semaphore::new(limit)),
            queue_timeout: ZSTD_QUEUE_TIMEOUT,
        }
    }

    pub(crate) async fn decode(&self, bytes: Bytes) -> Result<Bytes, ZstdDecodeError> {
        self.run(move || decode(bytes)).await?
    }

    async fn run<F, T>(&self, task: F) -> Result<T, ZstdDecodeError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let permit = tokio::time::timeout(
            self.queue_timeout,
            Arc::clone(&self.permits).acquire_owned(),
        )
        .await
        .map_err(|_| ZstdDecodeError::Overloaded)?
        .expect("the private zstd semaphore is never closed");
        self.lifecycle
            .spawn_blocking(move || {
                let _permit = permit;
                task()
            })
            .await
            .map_err(|_| ZstdDecodeError::TaskFailed)
    }

    #[cfg(test)]
    fn with_queue_timeout(mut self, queue_timeout: Duration) -> Self {
        self.queue_timeout = queue_timeout;
        self
    }
}

fn decode(bytes: Bytes) -> Result<Bytes, ZstdDecodeError> {
    let mut output = PayloadBuffer::with_capacity_hint(
        Some(bytes.len()),
        STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES,
    )
    .map_err(map_buffer_error)?;
    let mut decoder = zstd::stream::Decoder::new(bytes.as_ref()).map_err(map_decode_error)?;
    decoder
        .window_log_max(ZSTD_MAX_WINDOW_LOG)
        .map_err(map_decode_error)?;
    std::io::copy(&mut decoder, &mut output).map_err(map_decode_error)?;
    Ok(output.freeze().into_bytes())
}

fn map_buffer_error(error: PayloadBufferError) -> ZstdDecodeError {
    match error {
        PayloadBufferError::TooLarge => ZstdDecodeError::TooLarge,
        PayloadBufferError::AllocationFailed => ZstdDecodeError::AllocationFailed,
    }
}

fn map_decode_error(error: std::io::Error) -> ZstdDecodeError {
    match error.kind() {
        ErrorKind::WriteZero => ZstdDecodeError::TooLarge,
        ErrorKind::OutOfMemory => ZstdDecodeError::AllocationFailed,
        _ => ZstdDecodeError::Invalid,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ZstdDecodeError {
    Invalid,
    TooLarge,
    AllocationFailed,
    Overloaded,
    TaskFailed,
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        task::Poll,
        time::Duration,
    };

    use any2api_runtime::api::ProcessLifecycle;
    use axum::body::Bytes;

    use super::{
        STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES, ZstdDecodeError, ZstdDecoder, decode,
        default_decode_limit,
    };

    const TEST_DECODE_LIMIT: usize = 2;

    #[test]
    fn decode_limit_scales_with_parallelism_but_never_drops_below_two() {
        assert!(default_decode_limit() >= 2);
    }

    #[test]
    fn valid_body_is_decoded_and_damaged_body_is_rejected() {
        let encoded =
            zstd::stream::encode_all(&b"{\"model\":\"gpt\"}"[..], 3).expect("encode zstd");
        assert_eq!(
            decode(Bytes::from(encoded)).expect("decode zstd"),
            Bytes::from_static(b"{\"model\":\"gpt\"}")
        );
        assert_eq!(
            decode(Bytes::from_static(b"not-zstd")),
            Err(ZstdDecodeError::Invalid)
        );
    }

    #[test]
    fn concatenated_frames_are_decoded_and_trailing_garbage_is_rejected() {
        let first = zstd::stream::encode_all(&b"first"[..], 3).expect("encode first frame");
        let second = zstd::stream::encode_all(&b"second"[..], 3).expect("encode second frame");
        let mut concatenated = first.clone();
        concatenated.extend_from_slice(&second);
        assert_eq!(
            decode(Bytes::from(concatenated)).expect("decode concatenated frames"),
            Bytes::from_static(b"firstsecond")
        );

        let mut damaged = first;
        damaged.extend_from_slice(b"trailing garbage");
        assert_eq!(decode(Bytes::from(damaged)), Err(ZstdDecodeError::Invalid));
    }

    #[test]
    fn decompressed_output_over_the_public_limit_is_rejected() {
        let oversized = vec![b'a'; STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES + 1];
        let encoded = zstd::stream::encode_all(oversized.as_slice(), 1).expect("encode zstd");
        assert_eq!(decode(Bytes::from(encoded)), Err(ZstdDecodeError::TooLarge));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_blocking_work_is_limited() {
        let lifecycle = ProcessLifecycle::new();
        let decoder = ZstdDecoder::with_limit(lifecycle.clone(), TEST_DECODE_LIMIT);
        let (started_sender, mut started_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut releases = Vec::new();
        let mut tasks = Vec::new();

        for id in 0..TEST_DECODE_LIMIT {
            let decoder = decoder.clone();
            let started_sender = started_sender.clone();
            let (release_sender, release_receiver) = mpsc::channel();
            releases.push(Some(release_sender));
            tasks.push(tokio::spawn(async move {
                decoder
                    .run(move || {
                        started_sender.send(id).expect("started receiver");
                        release_receiver.recv().expect("release blocking work");
                    })
                    .await
                    .expect("blocking work")
            }));
        }
        drop(started_sender);

        let mut started_ids = Vec::new();
        for _ in 0..TEST_DECODE_LIMIT {
            started_ids.push(
                tokio::time::timeout(Duration::from_secs(1), started_receiver.recv())
                    .await
                    .expect("blocking work should start")
                    .expect("started id"),
            );
        }
        assert_eq!(lifecycle.background_task_count(), TEST_DECODE_LIMIT);

        let queued_started = Arc::new(AtomicBool::new(false));
        let queued = {
            let queued_started = Arc::clone(&queued_started);
            decoder.run(move || queued_started.store(true, Ordering::Release))
        };
        tokio::pin!(queued);
        assert!(matches!(
            futures_util::poll!(queued.as_mut()),
            Poll::Pending
        ));
        assert!(!queued_started.load(Ordering::Acquire));

        releases[started_ids[0]]
            .take()
            .expect("first release")
            .send(())
            .expect("release first blocking work");
        tokio::time::timeout(Duration::from_secs(1), queued.as_mut())
            .await
            .expect("queued work should start")
            .expect("queued blocking work");
        assert!(queued_started.load(Ordering::Acquire));

        for release in releases.into_iter().flatten() {
            release.send(()).expect("release blocking work");
        }
        for task in tasks {
            task.await.expect("join decode waiter");
        }
        assert_eq!(lifecycle.background_task_count(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelling_a_waiter_does_not_start_its_blocking_work() {
        let lifecycle = ProcessLifecycle::new();
        let decoder = ZstdDecoder::with_limit(lifecycle, 1);
        let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let first_decoder = decoder.clone();
        let first = tokio::spawn(async move {
            first_decoder
                .run(move || {
                    started_sender.send(()).expect("started receiver");
                    release_receiver.recv().expect("release first decode");
                })
                .await
                .expect("first blocking work")
        });
        started_receiver.await.expect("first blocking work started");

        let second_started = Arc::new(AtomicBool::new(false));
        {
            let second_started = Arc::clone(&second_started);
            let second = decoder.run(move || {
                second_started.store(true, Ordering::Release);
            });
            tokio::pin!(second);
            assert!(matches!(
                futures_util::poll!(second.as_mut()),
                Poll::Pending
            ));
        }

        release_sender.send(()).expect("release first decode");
        first.await.expect("join first waiter");
        assert!(!second_started.load(Ordering::Acquire));
        assert_eq!(decoder.permits.available_permits(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn started_work_remains_tracked_after_waiter_cancellation_and_force() {
        let lifecycle = ProcessLifecycle::new();
        let decoder = ZstdDecoder::with_limit(lifecycle.clone(), 1);
        let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let running_decoder = decoder.clone();
        let waiter = tokio::spawn(async move {
            running_decoder
                .run(move || {
                    started_sender.send(()).expect("started receiver");
                    release_receiver.recv().expect("release tracked decode");
                })
                .await
                .expect("tracked blocking work")
        });
        started_receiver.await.expect("blocking work started");

        waiter.abort();
        assert!(
            waiter
                .await
                .expect_err("waiter should be cancelled")
                .is_cancelled()
        );
        lifecycle.force();
        lifecycle.close_background_tasks();
        assert_eq!(lifecycle.background_task_count(), 1);
        assert_eq!(decoder.permits.available_permits(), 0);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(25),
                lifecycle.wait_for_background_tasks()
            )
            .await
            .is_err()
        );

        release_sender.send(()).expect("release tracked decode");
        tokio::time::timeout(
            Duration::from_secs(1),
            lifecycle.wait_for_background_tasks(),
        )
        .await
        .expect("tracked decode should finish");
        assert_eq!(lifecycle.background_task_count(), 0);
        assert_eq!(decoder.permits.available_permits(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn queued_work_times_out_as_overloaded_without_starting() {
        let lifecycle = ProcessLifecycle::new();
        let decoder =
            ZstdDecoder::with_limit(lifecycle, 1).with_queue_timeout(Duration::from_millis(25));
        let (started_sender, started_receiver) = tokio::sync::oneshot::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let first_decoder = decoder.clone();
        let first = tokio::spawn(async move {
            first_decoder
                .run(move || {
                    started_sender.send(()).expect("started receiver");
                    release_receiver.recv().expect("release first decode");
                })
                .await
                .expect("first blocking work")
        });
        started_receiver.await.expect("first blocking work started");

        let second_started = Arc::new(AtomicBool::new(false));
        let timed_out = {
            let second_started = Arc::clone(&second_started);
            decoder
                .run(move || second_started.store(true, Ordering::Release))
                .await
        };
        assert_eq!(
            timed_out.expect_err("queued work"),
            ZstdDecodeError::Overloaded
        );
        assert!(!second_started.load(Ordering::Acquire));

        release_sender.send(()).expect("release first decode");
        first.await.expect("join first waiter");
        assert_eq!(decoder.permits.available_permits(), 1);
    }
}
