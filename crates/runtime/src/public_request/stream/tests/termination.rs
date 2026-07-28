use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::RetrySafety;
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
};
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use tokio::time::timeout;

use super::core::{generation_permit, guarded_body};

const COMPACTION_ITEM: &[u8] = b"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction_summary\",\"encrypted_content\":\"opaque\",\"future_extension\":{\"kept\":true}}}\n\n";
const COMPLETED: &[u8] = b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_compact\"},\"future_terminal_field\":true}\n\n";

#[tokio::test]
async fn terminal_frame_ends_a_held_upstream_after_preserving_compaction_events() {
    let (binding, permit) = generation_permit();
    let upstream_dropped = Arc::new(AtomicBool::new(false));
    let mut chunk = Vec::from(COMPACTION_ITEM);
    chunk.extend_from_slice(COMPLETED);
    let upstream: BoxByteStream = Box::pin(DropObservedStream::new(
        stream::iter([Ok::<_, TransportError>(Bytes::from(chunk))]).chain(stream::pending()),
        Arc::clone(&upstream_dropped),
    ));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert_eq!(
        body.next()
            .await
            .expect("compaction item")
            .expect("compaction bytes"),
        COMPACTION_ITEM
    );
    assert_eq!(
        body.next()
            .await
            .expect("terminal frame")
            .expect("terminal bytes"),
        COMPLETED
    );
    assert_eq!(binding.in_flight(), 1);
    assert!(upstream_dropped.load(Ordering::Acquire));
    let end = timeout(Duration::from_millis(100), body.next())
        .await
        .expect("terminal frame must end the body without upstream EOF");
    assert!(end.is_none());
    assert_eq!(binding.in_flight(), 0);
}

struct DropObservedStream {
    inner: BoxByteStream,
    dropped: Arc<AtomicBool>,
}

impl DropObservedStream {
    fn new(
        inner: impl futures_util::Stream<Item = Result<Bytes, TransportError>> + Send + 'static,
        dropped: Arc<AtomicBool>,
    ) -> Self {
        Self {
            inner: Box::pin(inner),
            dropped,
        }
    }
}

impl futures_util::Stream for DropObservedStream {
    type Item = Result<Bytes, TransportError>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(context)
    }
}

impl Drop for DropObservedStream {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

#[tokio::test]
async fn transport_error_after_a_terminal_frame_is_ignored() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(COMPLETED)),
        Err(TransportError::new(
            TransportErrorStage::ReadBody,
            TransportFailureScope::Endpoint,
            RetrySafety::Ambiguous,
            "error after terminal event",
        )),
    ]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert_eq!(
        body.next()
            .await
            .expect("terminal frame")
            .expect("terminal bytes"),
        COMPLETED
    );
    assert_eq!(binding.in_flight(), 1);
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn eof_before_a_responses_terminal_event_is_an_incomplete_stream() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_incomplete\"}}\n\n",
    ))]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert!(body.next().await.expect("created frame").is_ok());
    let error = body
        .next()
        .await
        .expect("missing terminal event must produce a body error")
        .expect_err("EOF before terminal must not succeed");
    assert!(error.to_string().contains("terminal event"));
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn done_sentinel_does_not_replace_a_responses_terminal_event() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream =
        Box::pin(stream::iter([Ok(Bytes::from_static(b"data: [DONE]\n\n"))]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("primed stream")
        .into_stream();

    assert_eq!(
        body.next()
            .await
            .expect("done sentinel")
            .expect("done bytes"),
        b"data: [DONE]\n\n".as_slice()
    );
    let error = body
        .next()
        .await
        .expect("missing terminal event must produce a body error")
        .expect_err("done sentinel without terminal must not succeed");
    assert!(error.to_string().contains("terminal event"));
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn terminal_frame_without_a_trailing_blank_line_finishes_on_eof() {
    let (binding, permit) = generation_permit();
    let terminal =
        b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{}}";
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(terminal))]));
    let mut body = guarded_body(upstream, permit)
        .prime()
        .await
        .expect("terminal frame must be flushed at EOF")
        .into_stream();

    assert_eq!(
        body.next()
            .await
            .expect("terminal frame")
            .expect("terminal bytes"),
        terminal.as_slice()
    );
    assert_eq!(binding.in_flight(), 1);
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}
