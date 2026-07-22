use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde_json::json;

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse,
    },
    json_codec,
    sse::rewrite_known_model,
};

mod telemetry;

#[derive(Debug, Default)]
pub struct AnthropicMessagesAdapter;

impl AnthropicMessagesAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProtocolAdapter for AnthropicMessagesAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::AnthropicMessages
    }

    fn decode_ingress_request(
        &self,
        request: IngressRequest,
    ) -> Result<DecodedRequest, ProtocolError> {
        json_codec::decode_request(request, self.dialect())
    }

    fn encode_upstream_request(
        &self,
        operation: ProtocolOperation,
        headers: HeaderMap,
        payload: AdapterPayload,
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
        Ok(DecodedUpstreamResponse {
            status: response.status,
            headers: response.headers,
            telemetry: telemetry::response(&response.body),
            payload: AdapterPayload::RawJson(response.body),
        })
    }

    fn decode_upstream_event(&self, frame: SseFrame) -> Result<AdapterEvent, ProtocolError> {
        let telemetry = telemetry::event(&frame.0);
        Ok(AdapterEvent::new(frame.0, telemetry))
    }

    fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
    ) -> Result<EgressResponse, ProtocolError> {
        let AdapterPayload::RawJson(body) = response.payload;
        Ok(EgressResponse {
            status: response.status,
            headers: response.headers,
            body,
        })
    }

    fn encode_egress_event(
        &self,
        event: AdapterEvent,
        public_model: &str,
    ) -> Result<SseFrame, ProtocolError> {
        rewrite_known_model(SseFrame(event.into_bytes()), public_model)
    }

    fn error_response(&self, error: &PublicError) -> EgressResponse {
        let error_type = error_type(error.code);
        let mut response = json_response(
            public_error_status(error.code),
            json!({
                "type": "error",
                "error": {
                    "type": error_type,
                    "message": error.message
                }
            }),
        );
        insert_retry_after(&mut response.headers, error.retry_after_seconds);
        response
    }
}

fn error_type(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "authentication_error",
        PublicErrorCode::InvalidRequest | PublicErrorCode::MethodNotAllowed => {
            "invalid_request_error"
        }
        PublicErrorCode::PublicApiNotFound => "not_found_error",
        PublicErrorCode::ModelNotFound
        | PublicErrorCode::NoRoute
        | PublicErrorCode::UpstreamNotFound => "not_found_error",
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalConcurrencyLimit => {
            "rate_limit_error"
        }
        PublicErrorCode::SessionBindingLost => "invalid_request_error",
        PublicErrorCode::UpstreamError => "api_error",
        PublicErrorCode::InternalError => "api_error",
    }
}

fn public_error_status(code: PublicErrorCode) -> StatusCode {
    match code {
        PublicErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        PublicErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        PublicErrorCode::PublicApiNotFound => StatusCode::NOT_FOUND,
        PublicErrorCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
        PublicErrorCode::ModelNotFound
        | PublicErrorCode::NoRoute
        | PublicErrorCode::UpstreamNotFound => StatusCode::NOT_FOUND,
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalConcurrencyLimit => {
            StatusCode::TOO_MANY_REQUESTS
        }
        PublicErrorCode::SessionBindingLost => StatusCode::CONFLICT,
        PublicErrorCode::UpstreamError => StatusCode::BAD_GATEWAY,
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
    use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
    use bytes::Bytes;
    use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri};
    use serde_json::{Value, json};

    use super::AnthropicMessagesAdapter;
    use crate::api::{IngressRequest, ProtocolAdapter, SseFrame};

    #[test]
    fn messages_rewrites_model_and_uses_anthropic_error_shape() {
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
            .expect("decoded request");
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                decoded.headers,
                decoded.payload,
                "claude-upstream",
            )
            .expect("encoded request");
        let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
        assert_eq!(body["model"], "claude-upstream");
        assert_eq!(body["future_field"], 42);
        assert_eq!(encoded.headers["anthropic-beta"], "messages-2024-09-04");

        let response = adapter.error_response(&PublicError::new(
            PublicErrorCode::LocalConcurrencyLimit,
            "full",
        ));
        let body: Value = serde_json::from_slice(&response.body).expect("error JSON");
        assert_eq!(body["type"], "error");
        assert_eq!(body["error"]["type"], "rate_limit_error");
    }

    #[test]
    fn count_tokens_preserves_fields_and_upstream_not_found_is_compatible() {
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
            .expect("decoded count tokens request");
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                decoded.headers,
                decoded.payload,
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
                .is_err()
        );

        let response = adapter.error_response(&PublicError::new(
            PublicErrorCode::UpstreamNotFound,
            "unavailable",
        ));
        let body: Value = serde_json::from_slice(&response.body).expect("error JSON");
        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["type"], "not_found_error");

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
}
