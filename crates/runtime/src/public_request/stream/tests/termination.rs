use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::{ProtocolOperation, PublicErrorCode, RetrySafety, SettingsConfiguration};
use any2api_protocol::{
    AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
    api::{ProtocolAdapter, StreamRetryReason},
};
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
};
use bytes::Bytes;
use futures_util::{StreamExt, stream};
use tokio::time::timeout;

use super::super::StreamPrimeFailure;
use super::core::{
    generation_permit, guarded_body, guarded_body_for_adapter, guarded_body_for_adapter_with_health,
};
use crate::health::{AttemptHealth, ReliabilityPolicy};

const COMPACTION_ITEM: &[u8] = b"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction_summary\",\"encrypted_content\":\"opaque\",\"future_extension\":{\"kept\":true}}}\n\n";
const COMPLETED: &[u8] = b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_compact\"},\"future_terminal_field\":true}\n\n";
type ProtocolStreamCase = (
    &'static str,
    Arc<dyn ProtocolAdapter>,
    ProtocolOperation,
    &'static [u8],
);

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
async fn lifecycle_only_eof_fails_before_committing_a_responses_stream() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_incomplete\"}}\n\n",
    ))]));
    match guarded_body(upstream, permit).prime_attempt().await {
        Err(StreamPrimeFailure::Public(error)) => {
            assert_eq!(error.code(), PublicErrorCode::UpstreamError);
        }
        Err(StreamPrimeFailure::Retryable(reason)) => {
            panic!("truncated lifecycle stream must not be retryable: {reason:?}")
        }
        Ok(_) => panic!("truncated lifecycle stream must fail before commit"),
    }
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

#[tokio::test]
async fn every_streaming_protocol_rejects_eof_before_its_terminal_event() {
    let cases: Vec<ProtocolStreamCase> = vec![
        (
            "Messages",
            Arc::new(AnthropicMessagesAdapter::new()),
            ProtocolOperation::Messages,
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n",
        ),
        (
            "Chat Completions",
            Arc::new(OpenAiChatCompletionsAdapter::new()),
            ProtocolOperation::ChatCompletions,
            b"data: {\"model\":\"upstream\",\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
        ),
        (
            "Images",
            Arc::new(OpenAiImagesAdapter::new()),
            ProtocolOperation::ImagesGenerations,
            b"event: image_generation.partial_image\ndata: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"abc\"}\n\n",
        ),
    ];

    for (name, adapter, operation, partial) in cases {
        let (binding, permit) = generation_permit();
        let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(partial))]));
        let mut body = guarded_body_for_adapter(upstream, permit, adapter, operation)
            .prime()
            .await
            .unwrap_or_else(|error| panic!("{name} first event must commit: {error:?}"))
            .into_stream();

        assert!(
            body.next()
                .await
                .unwrap_or_else(|| panic!("{name} partial frame"))
                .is_ok()
        );
        let error = body
            .next()
            .await
            .unwrap_or_else(|| panic!("{name} missing terminal Body error"))
            .expect_err("EOF before terminal must fail");
        assert!(error.to_string().contains("terminal event"), "{name}");
        assert!(body.next().await.is_none(), "{name}");
        assert_eq!(binding.in_flight(), 0, "{name}");
    }
}

#[tokio::test]
async fn protocol_failure_events_are_forwarded_before_error_settlement() {
    let cases: Vec<ProtocolStreamCase> = vec![
        (
            "Messages",
            Arc::new(AnthropicMessagesAdapter::new()),
            ProtocolOperation::Messages,
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\"}}\n\n",
        ),
        (
            "Chat Completions",
            Arc::new(OpenAiChatCompletionsAdapter::new()),
            ProtocolOperation::ChatCompletions,
            b"event: error\ndata: {\"error\":{\"message\":\"busy\"}}\n\n",
        ),
        (
            "Images",
            Arc::new(OpenAiImagesAdapter::new()),
            ProtocolOperation::ImagesGenerations,
            b"event: error\ndata: {\"error\":{\"message\":\"generation failed\"}}\n\n",
        ),
    ];

    for (name, adapter, operation, failure) in cases {
        let (binding, permit) = generation_permit();
        let upstream: BoxByteStream =
            Box::pin(stream::iter([Ok(Bytes::from_static(failure))]).chain(stream::pending()));
        let mut body = guarded_body_for_adapter(upstream, permit, adapter, operation)
            .prime()
            .await
            .unwrap_or_else(|error| panic!("{name} failure event must be forwarded: {error:?}"))
            .into_stream();

        assert_eq!(
            body.next()
                .await
                .unwrap_or_else(|| panic!("{name} failure frame"))
                .expect("failure event bytes"),
            failure,
            "{name}"
        );
        assert!(body.next().await.is_none(), "{name}");
        assert_eq!(binding.in_flight(), 0, "{name}");
    }
}

#[tokio::test]
async fn lifecycle_frames_then_exact_overload_remain_uncommitted_for_retry() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_rejected\"}}\n\nevent: response.in_progress\ndata: {\"type\":\"response.in_progress\",\"response\":{\"id\":\"resp_rejected\"}}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"service_unavailable_error\",\"code\":\"server_is_overloaded\",\"message\":\"busy\"}}\n\n",
    ))]));

    match guarded_body(upstream, permit).prime_attempt().await {
        Err(StreamPrimeFailure::Retryable(StreamRetryReason::Overloaded)) => {}
        Ok(_) => panic!("pre-content overload must not commit the stream"),
        Err(StreamPrimeFailure::Public(error)) => {
            panic!("pre-content overload must remain retryable: {error:?}")
        }
    }
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn exact_overload_after_semantic_output_is_forwarded_without_retry() {
    let (binding, permit) = generation_permit();
    let content = b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"hello\"}\n\n";
    let failure = b"event: error\ndata: {\"type\":\"error\",\"error\":{\"code\":\"server_is_overloaded\",\"message\":\"busy\"}}\n\n";
    let mut bytes = Vec::from(content);
    bytes.extend_from_slice(failure);
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from(bytes))]));
    let mut body = guarded_body(upstream, permit)
        .prime_attempt()
        .await
        .expect("semantic output commits the attempt")
        .into_stream();

    assert_eq!(
        body.next().await.expect("content").expect("bytes"),
        content.as_slice()
    );
    assert_eq!(
        body.next().await.expect("failure").expect("bytes"),
        failure.as_slice()
    );
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn stream_health_succeeds_only_after_a_successful_terminal_event() {
    const DELTA: &[u8] = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n";
    const FAILED: &[u8] =
        b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n\n";
    const COMPLETED: &[u8] = b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";

    for (name, terminal, should_clear_health) in
        [("failed", FAILED, false), ("completed", COMPLETED, true)]
    {
        let (binding, permit) = generation_permit();
        binding
            .generation()
            .health()
            .record_quota_exhaustion(Duration::from_secs(60), None, None);
        let health = AttemptHealth::new(
            Arc::clone(binding.generation()),
            "upstream".into(),
            None,
            None,
            ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability()),
        );
        let mut bytes = Vec::from(DELTA);
        bytes.extend_from_slice(terminal);
        let upstream: BoxByteStream =
            Box::pin(stream::iter([Ok(Bytes::from(bytes))]).chain(stream::pending()));
        let mut body = guarded_body_for_adapter_with_health(
            upstream,
            permit,
            Arc::new(AnthropicMessagesAdapter::new()),
            ProtocolOperation::Messages,
            health,
        )
        .prime()
        .await
        .unwrap_or_else(|error| panic!("{name} stream must prime: {error:?}"))
        .into_stream();

        assert!(
            binding.generation().health().quota_exhaustion().is_some(),
            "the first ordinary event must not settle health"
        );
        assert_eq!(
            body.next()
                .await
                .expect("content event")
                .expect("content bytes"),
            DELTA
        );
        assert_eq!(
            body.next()
                .await
                .expect("terminal event")
                .expect("terminal bytes"),
            terminal
        );
        assert!(body.next().await.is_none());
        assert_eq!(
            binding.generation().health().quota_exhaustion().is_none(),
            should_clear_health,
            "{name} stream health settlement"
        );
        assert_eq!(binding.in_flight(), 0);
    }
}
