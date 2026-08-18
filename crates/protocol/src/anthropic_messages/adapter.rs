use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode};
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde_json::json;

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedResponsePayload,
        DecodedUpstreamResponse, EgressResponse, EncodedUpstreamRequest, IngressRequest,
        ProtocolAdapter, SseFrame, StreamCompletionPolicy, UpstreamResponse,
    },
    json_codec,
    sse::{parse_event_payload, rewrite_known_model},
};

use super::{telemetry, termination};

#[derive(Debug, Default)]
pub struct AnthropicMessagesAdapter;

impl AnthropicMessagesAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolAdapter for AnthropicMessagesAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::AnthropicMessages
    }

    fn stream_completion_policy(&self, operation: ProtocolOperation) -> StreamCompletionPolicy {
        if operation == ProtocolOperation::Messages {
            StreamCompletionPolicy::TerminalEventRequired
        } else {
            StreamCompletionPolicy::EofAllowed
        }
    }

    async fn decode_ingress_request(
        &self,
        request: IngressRequest,
    ) -> Result<DecodedRequest, ProtocolError> {
        json_codec::decode_request(request, self.dialect())
    }

    fn encode_upstream_request(
        &self,
        operation: ProtocolOperation,
        headers: &HeaderMap,
        payload: &AdapterPayload,
        upstream_model: &str,
    ) -> Result<EncodedUpstreamRequest, ProtocolError> {
        if !matches!(
            operation,
            ProtocolOperation::Messages | ProtocolOperation::MessagesCountTokens
        ) {
            return Err(ProtocolError::Unsupported(format!("{operation:?}")));
        }
        json_codec::encode_request(operation, headers, payload, upstream_model)
    }

    fn decode_upstream_response(
        &self,
        response: UpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        let parsed = json_codec::parse_response_body(&response.body)?;
        Ok(DecodedUpstreamResponse {
            status: response.status,
            headers: response.headers,
            telemetry: telemetry::response(&parsed),
            payload: DecodedResponsePayload::StructuredJson(parsed),
        })
    }

    fn decode_direct_upstream_response(
        &self,
        response: UpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        json_codec::decode_direct_response(response, telemetry::raw_response)
    }

    fn decode_upstream_event(&self, frame: SseFrame) -> Result<AdapterEvent, ProtocolError> {
        let payload = parse_event_payload(&frame.0)?;
        let telemetry = telemetry::event(&payload);
        let termination = termination::classify(&payload);
        let upstream_failure = crate::upstream_failure::anthropic_stream(&payload, termination);
        Ok(AdapterEvent::new(frame.0, telemetry, payload)
            .with_termination(termination)
            .with_upstream_failure(upstream_failure))
    }

    fn buffered_upstream_failure(
        &self,
        response: &UpstreamResponse,
    ) -> Option<crate::api::ProtocolUpstreamFailureEvidence> {
        response
            .status
            .is_success()
            .then(|| crate::upstream_failure::anthropic_buffered(&response.body))
            .flatten()
    }

    fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
        public_model: &str,
    ) -> Result<EgressResponse, ProtocolError> {
        json_codec::encode_response(response, public_model)
    }

    fn encode_egress_event(
        &self,
        event: AdapterEvent,
        public_model: &str,
    ) -> Result<SseFrame, ProtocolError> {
        let (bytes, payload) = event.into_parts();
        rewrite_known_model(bytes, payload, public_model)
    }

    fn error_response(&self, error: &PublicError) -> EgressResponse {
        let error_type = error_type(error.code());
        let mut response = json_response(
            public_error_status(error.code()),
            json!({
                "type": "error",
                "error": {
                    "type": error_type,
                    "message": error.client_message()
                }
            }),
        );
        insert_retry_after(&mut response.headers, error.retry_after_seconds());
        response
    }
}

fn error_type(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "authentication_error",
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::MethodNotAllowed
        | PublicErrorCode::ModelNotFound => "invalid_request_error",
        PublicErrorCode::PayloadTooLarge => "request_too_large",
        PublicErrorCode::PublicApiNotFound => "not_found_error",
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalRateLimit => {
            "rate_limit_error"
        }
        PublicErrorCode::SessionBindingLost => "invalid_request_error",
        PublicErrorCode::UpstreamError | PublicErrorCode::GatewayTimeout => "api_error",
        PublicErrorCode::InternalError => "api_error",
    }
}

fn public_error_status(code: PublicErrorCode) -> StatusCode {
    match code {
        PublicErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        PublicErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        PublicErrorCode::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        PublicErrorCode::PublicApiNotFound => StatusCode::NOT_FOUND,
        PublicErrorCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        PublicErrorCode::ModelNotFound => StatusCode::BAD_REQUEST,
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalRateLimit => {
            StatusCode::TOO_MANY_REQUESTS
        }
        PublicErrorCode::SessionBindingLost => StatusCode::CONFLICT,
        PublicErrorCode::UpstreamError => StatusCode::BAD_GATEWAY,
        PublicErrorCode::GatewayTimeout => StatusCode::GATEWAY_TIMEOUT,
        PublicErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn json_response(status: StatusCode, value: serde_json::Value) -> EgressResponse {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    EgressResponse {
        status,
        headers,
        body: Bytes::from(serde_json::to_vec(&value).expect("JSON value encodes")),
    }
}

fn insert_retry_after(headers: &mut HeaderMap, seconds: Option<u64>) {
    if let Some(seconds) = seconds
        && let Ok(value) = HeaderValue::from_str(&seconds.to_string())
    {
        headers.insert(header::RETRY_AFTER, value);
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode, RetrySafety};
    use bytes::Bytes;
    use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
    use serde_json::{Value, json};

    use super::AnthropicMessagesAdapter;
    use crate::api::{
        IngressRequest, ProtocolAdapter, ProtocolRetryDelayBasis, SseFrame, StreamCompletionPolicy,
        StreamTermination,
    };

    #[tokio::test]
    async fn messages_rewrites_model_and_uses_anthropic_error_shape() {
        let adapter = AnthropicMessagesAdapter::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            "anthropic-beta",
            HeaderValue::from_static("messages-2024-09-04"),
        );
        let decoded = adapter
            .decode_ingress_request(IngressRequest {
                method: Method::POST,
                uri: Uri::from_static("/v1/messages"),
                headers,
                body: Bytes::from(
                    serde_json::to_vec(&json!({
                        "model": "public",
                        "messages": [],
                        "future_field": 42
                    }))
                    .expect("JSON"),
                ),
                operation: ProtocolOperation::Messages,
            })
            .await
            .expect("decoded request");
        assert_eq!(
            decoded.client_headers["anthropic-beta"],
            "messages-2024-09-04"
        );
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                &decoded.headers,
                &decoded.payload,
                "claude-upstream",
            )
            .expect("encoded request");
        let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
        assert_eq!(body["model"], "claude-upstream");
        assert_eq!(body["future_field"], 42);
        assert!(!encoded.headers.contains_key("anthropic-beta"));

        let response =
            adapter.error_response(&PublicError::new(PublicErrorCode::LocalRateLimit, "full"));
        let body: Value = serde_json::from_slice(&response.body).expect("error JSON");
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "rate_limit_error");

        let missing_model = adapter.error_response(&PublicError::new(
            PublicErrorCode::ModelNotFound,
            "model is unavailable",
        ));
        let body: Value = serde_json::from_slice(&missing_model.body).expect("error JSON");
        assert_eq!(missing_model.status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["type"], "invalid_request_error");
    }

    #[tokio::test]
    async fn count_tokens_preserves_fields_and_rejects_streaming() {
        let adapter = AnthropicMessagesAdapter::new();
        let decoded = adapter
            .decode_ingress_request(IngressRequest {
                method: Method::POST,
                uri: Uri::from_static("/v1/messages/count_tokens"),
                headers: HeaderMap::new(),
                body: Bytes::from_static(
                    br#"{"model":"public","messages":[],"system":"test","tools":[],"future":true}"#,
                ),
                operation: ProtocolOperation::MessagesCountTokens,
            })
            .await
            .expect("decoded count tokens request");
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                &decoded.headers,
                &decoded.payload,
                "claude-upstream",
            )
            .expect("encoded count tokens request");
        let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
        assert_eq!(body["model"], "claude-upstream");
        assert_eq!(body["system"], "test");
        assert_eq!(body["tools"], json!([]));
        assert_eq!(body["future"], true);
        assert!(
            adapter
                .decode_ingress_request(IngressRequest {
                    method: Method::POST,
                    uri: Uri::from_static("/v1/messages/count_tokens"),
                    headers: HeaderMap::new(),
                    body: Bytes::from_static(br#"{"model":"public","stream":true}"#),
                    operation: ProtocolOperation::MessagesCountTokens,
                })
                .await
                .is_err()
        );

        let method = adapter.error_response(&PublicError::new(
            PublicErrorCode::MethodNotAllowed,
            "wrong method",
        ));
        let body: Value = serde_json::from_slice(&method.body).expect("error JSON");
        assert_eq!(method.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "invalid_request_error");
    }

    #[test]
    fn messages_stream_rewrites_the_public_model() {
        let adapter = AnthropicMessagesAdapter::new();
        let event = adapter
            .decode_upstream_event(SseFrame(Bytes::from_static(
                b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"upstream\"}}\n\n",
            )))
            .expect("decoded event");
        let frame = adapter
            .encode_egress_event(event, "public")
            .expect("encoded event");
        assert!(String::from_utf8_lossy(&frame.0).contains(r#""model":"public""#));
    }

    #[test]
    fn messages_stream_requires_and_classifies_terminal_events() {
        let adapter = AnthropicMessagesAdapter::new();
        assert_eq!(
            adapter.stream_completion_policy(ProtocolOperation::Messages),
            StreamCompletionPolicy::TerminalEventRequired
        );
        assert_eq!(
            adapter
                .decode_upstream_event(SseFrame(Bytes::from_static(
                    b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
                )))
                .expect("message_stop")
                .termination(),
            StreamTermination::Completed
        );
        let overload = adapter
            .decode_upstream_event(SseFrame(Bytes::from_static(
                b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n\n",
            )))
            .expect("error event");
        assert_eq!(overload.termination(), StreamTermination::Failed);
        assert!(overload.upstream_failure().is_some());
        let rate_limit = adapter
            .decode_upstream_event(SseFrame(Bytes::from_static(
                b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}\n\n",
            )))
            .expect("rate limit event");
        let evidence = rate_limit
            .upstream_failure()
            .expect("rate limit failure evidence");
        assert_eq!(
            evidence.retry_safety_override(),
            Some(RetrySafety::RejectedBeforeExecution)
        );
        assert_eq!(
            evidence.retry_delay_basis(),
            ProtocolRetryDelayBasis::CandidateAttempts
        );
        assert_eq!(
            adapter
                .decode_upstream_event(SseFrame(Bytes::from_static(
                    b"event: ping\ndata: {\"type\":\"ping\"}\n\n",
                )))
                .expect("ping")
                .termination(),
            StreamTermination::None
        );
    }
}
