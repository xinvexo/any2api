use std::{sync::Arc, time::Duration};

use any2api_domain::{ProtocolOperation, RetrySafety, SettingsConfiguration};
use any2api_protocol::{AnthropicMessagesAdapter, api::ProtocolUpstreamFailureEvidence};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{StreamExt, stream};

use super::core::{generation_permit, guarded_body, guarded_body_for_adapter_with_health};
use crate::health::{AttemptHealth, ReliabilityPolicy};

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
async fn protocol_retry_safety_override_applies_only_before_semantic_output() {
    let (_binding, permit) = generation_permit();
    let body = guarded_body(Box::pin(stream::empty()), permit);
    let evidence = ProtocolUpstreamFailureEvidence::new(Bytes::from_static(
        br#"{"type":"error","error":{"code":"server_is_overloaded","message":"busy"}}"#,
    ))
    .with_retry_safety_override(RetrySafety::RejectedBeforeExecution);

    let presemantic = body.classify_upstream_failure(&evidence, true);
    let postsemantic = body.classify_upstream_failure(&evidence, false);
    assert_eq!(
        presemantic.classification().retry_safety(),
        RetrySafety::RejectedBeforeExecution
    );
    assert_eq!(
        postsemantic.classification().retry_safety(),
        RetrySafety::Ambiguous
    );
}

#[tokio::test]
async fn terminal_frame_settles_health_before_the_body_can_be_dropped() {
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
        drop(body);
        assert_eq!(
            binding.generation().health().quota_exhaustion().is_none(),
            should_clear_health,
            "{name} stream health settlement"
        );
        assert_eq!(binding.in_flight(), 0);
    }
}
