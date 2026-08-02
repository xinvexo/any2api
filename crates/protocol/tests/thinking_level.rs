use any2api_domain::ProtocolOperation;
use any2api_protocol::{
    AnthropicMessagesAdapter,
    api::{IngressRequest, ProtocolAdapter},
};
use bytes::Bytes;
use http::{HeaderMap, Method, Uri};

#[tokio::test]
async fn claude_adaptive_thinking_records_the_explicit_effort_level() {
    let decoded = AnthropicMessagesAdapter::new()
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/messages"),
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"model":"claude-opus-5","reasoning":{"effort":"low"},"reasoning_effort":"high","thinking":{"type":"adaptive"},"output_config":{"effort":"max"},"messages":[]}"#,
            ),
            operation: ProtocolOperation::Messages,
        })
        .await
        .expect("decoded Claude request");

    assert_eq!(decoded.thinking_level.as_deref(), Some("max"));
}

#[tokio::test]
async fn claude_thinking_mode_and_budget_are_not_recorded_as_an_effort_level() {
    let decoded = AnthropicMessagesAdapter::new()
        .decode_ingress_request(IngressRequest {
            method: Method::POST,
            uri: Uri::from_static("/v1/messages"),
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"model":"claude-opus-5","thinking":{"type":"enabled","budget_tokens":8192},"messages":[]}"#,
            ),
            operation: ProtocolOperation::Messages,
        })
        .await
        .expect("decoded Claude request");

    assert_eq!(decoded.thinking_level, None);
}
