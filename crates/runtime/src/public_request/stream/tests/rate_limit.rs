use std::sync::Arc;

use any2api_domain::{ProtocolOperation, SettingsConfiguration};
use any2api_protocol::{AnthropicMessagesAdapter, api::StreamRetryReason};
use any2api_transport::api::BoxByteStream;
use bytes::Bytes;
use futures_util::{StreamExt, stream};

use super::super::StreamPrimeFailure;
use super::core::{
    generation_permit, guarded_body_for_adapter, guarded_body_for_adapter_with_health,
};
use crate::health::{AttemptHealth, ReliabilityPolicy};

#[tokio::test]
async fn anthropic_rate_limit_before_content_reselects_the_credential_model() {
    let (binding, permit) = generation_permit();
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(
        b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\nevent: ping\ndata: {\"type\":\"ping\"}\n\nevent: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\",\"message\":\"Concurrency limit exceeded for account\"}}\n\n",
    ))]));
    let health = AttemptHealth::new(
        Arc::clone(binding.generation()),
        "upstream".into(),
        None,
        None,
        ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability()),
    );

    match guarded_body_for_adapter_with_health(
        upstream,
        permit,
        Arc::new(AnthropicMessagesAdapter::new()),
        ProtocolOperation::Messages,
        health,
    )
    .prime_attempt()
    .await
    {
        Err(StreamPrimeFailure::Retryable(StreamRetryReason::RateLimited)) => {}
        Ok(_) => panic!("pre-content Anthropic rate limit must not commit the stream"),
        Err(StreamPrimeFailure::Retryable(reason)) => {
            panic!("unexpected retry reason: {reason:?}")
        }
        Err(StreamPrimeFailure::Public(error)) => {
            panic!("pre-content Anthropic rate limit must be retryable: {error:?}")
        }
    }
    assert_eq!(binding.generation().health().model_cooldown_count(), 1);
    assert_eq!(binding.in_flight(), 0);
}

#[tokio::test]
async fn anthropic_rate_limit_after_content_is_forwarded_without_retry() {
    let (binding, permit) = generation_permit();
    let content = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n";
    let failure =
        b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}\n\n";
    let mut bytes = Vec::from(content);
    bytes.extend_from_slice(failure);
    let upstream: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from(bytes))]));
    let mut body = guarded_body_for_adapter(
        upstream,
        permit,
        Arc::new(AnthropicMessagesAdapter::new()),
        ProtocolOperation::Messages,
    )
    .prime_attempt()
    .await
    .expect("content commits the attempt")
    .into_stream();

    assert_eq!(
        body.next().await.expect("content").expect("bytes"),
        content.as_slice()
    );
    assert_eq!(
        body.next().await.expect("rate limit event").expect("bytes"),
        failure.as_slice()
    );
    assert!(body.next().await.is_none());
    assert_eq!(binding.in_flight(), 0);
}
