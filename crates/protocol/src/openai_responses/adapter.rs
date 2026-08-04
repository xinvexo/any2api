use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode};
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde_json::{Value, json};

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressRequest, ProtocolAdapter, SseEventPayload, SseFrame,
        StreamCompletionPolicy, UpstreamResponse,
    },
    json_codec,
    sse::{parse_event_payload, rewrite_known_model},
};

use super::{replay_identity, telemetry, termination};

#[derive(Debug, Default)]
pub struct OpenAiResponsesAdapter;

impl OpenAiResponsesAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolAdapter for OpenAiResponsesAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiResponses
    }

    fn stream_completion_policy(&self, operation: ProtocolOperation) -> StreamCompletionPolicy {
        if operation == ProtocolOperation::Responses {
            StreamCompletionPolicy::TerminalEventRequired
        } else {
            StreamCompletionPolicy::EofAllowed
        }
    }

    async fn decode_ingress_request(
        &self,
        request: IngressRequest,
    ) -> Result<DecodedRequest, ProtocolError> {
        let mut decoded = json_codec::decode_request(request, self.dialect())?;
        replay_identity::normalize(&mut decoded.payload)?;
        Ok(decoded)
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
            ProtocolOperation::Responses | ProtocolOperation::ResponsesCompact
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
            body: Some(response.body),
            parsed,
        })
    }

    fn decode_upstream_event(&self, frame: SseFrame) -> Result<AdapterEvent, ProtocolError> {
        let payload = parse_event_payload(&frame.0)?;
        let telemetry = telemetry::event(&payload);
        let termination = termination::classify(&payload);
        Ok(AdapterEvent::new(frame.0, telemetry, payload).with_termination(termination))
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

    fn continuation_id_from_response(
        &self,
        operation: ProtocolOperation,
        response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        if operation != ProtocolOperation::Responses {
            return Ok(None);
        }
        optional_non_empty_id(response.parsed.get("id"), "response id")
    }

    fn continuation_id_from_event(
        &self,
        operation: ProtocolOperation,
        event: &AdapterEvent,
    ) -> Result<Option<String>, ProtocolError> {
        if operation != ProtocolOperation::Responses {
            return Ok(None);
        }
        let SseEventPayload::Json {
            event_name,
            data: value,
        } = event.payload()
        else {
            return Ok(None);
        };
        let is_created = event_name.as_deref() == Some("response.created")
            || value.get("type").and_then(Value::as_str) == Some("response.created");
        if !is_created {
            return Ok(None);
        }
        optional_non_empty_id(
            value.get("response").and_then(|value| value.get("id")),
            "response id",
        )?
        .map(Some)
        .ok_or_else(|| {
            ProtocolError::InvalidPayload("response.created is missing response.id".into())
        })
    }

    fn error_response(&self, error: &PublicError) -> EgressResponse {
        let code = error_code(error.code());
        let error_type = error_type(error.code());
        let param = error_param(error.code());
        let mut response = json_response(
            public_error_status(error.code()),
            json!({
                "error": {
                    "message": error.client_message(),
                    "type": error_type,
                    "param": param,
                    "code": code
                }
            }),
        );
        insert_retry_after(&mut response.headers, error.retry_after_seconds());
        response
    }
}

fn optional_non_empty_id(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, ProtocolError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| ProtocolError::InvalidPayload(format!("{field} must be a string")))?;
    let value = value.trim();
    if value.is_empty() {
        return Err(ProtocolError::InvalidPayload(format!(
            "{field} must not be empty"
        )));
    }
    Ok(Some(value.to_owned()))
}

fn error_type(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "authentication_error",
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::PayloadTooLarge
        | PublicErrorCode::PublicApiNotFound
        | PublicErrorCode::MethodNotAllowed
        | PublicErrorCode::ModelNotFound
        | PublicErrorCode::SessionBindingLost => "invalid_request_error",
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalRateLimit => {
            "rate_limit_error"
        }
        PublicErrorCode::UpstreamError
        | PublicErrorCode::GatewayTimeout
        | PublicErrorCode::InternalError => "server_error",
    }
}

fn error_code(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "unauthorized",
        PublicErrorCode::InvalidRequest => "invalid_request",
        PublicErrorCode::PayloadTooLarge => "payload_too_large",
        PublicErrorCode::PublicApiNotFound => "public_api_not_found",
        PublicErrorCode::MethodNotAllowed => "method_not_allowed",
        PublicErrorCode::ModelNotFound => "model_not_found",
        PublicErrorCode::NoAvailableCredential => "no_available_credential",
        PublicErrorCode::LocalRateLimit => "local_rate_limit",
        PublicErrorCode::SessionBindingLost => "session_binding_lost",
        PublicErrorCode::UpstreamError => "upstream_error",
        PublicErrorCode::GatewayTimeout => "gateway_timeout",
        PublicErrorCode::InternalError => "internal_error",
    }
}

fn error_param(code: PublicErrorCode) -> Option<&'static str> {
    (code == PublicErrorCode::ModelNotFound).then_some("model")
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
    use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
    use bytes::Bytes;
    use http::{HeaderMap, Method, Uri};
    use serde_json::{Value, json};

    use super::OpenAiResponsesAdapter;
    use crate::api::{
        AdapterPayload, DecodedUpstreamResponse, IngressRequest, ProtocolAdapter, SseFrame,
    };

    #[tokio::test]
    async fn preserves_unknown_fields_and_rewrites_the_upstream_model() {
        let adapter = OpenAiResponsesAdapter::new();
        let decoded = adapter
            .decode_ingress_request(IngressRequest {
                method: Method::POST,
                uri: Uri::from_static("/v1/responses"),
                headers: HeaderMap::new(),
                body: Bytes::from(
                    serde_json::to_vec(&json!({
                        "model": "public",
                        "stream": false,
                        "future_field": {"enabled": true}
                    }))
                    .expect("JSON"),
                ),
                operation: ProtocolOperation::Responses,
            })
            .await
            .expect("decoded request");
        assert_eq!(decoded.model.as_deref(), Some("public"));
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                &decoded.headers,
                &decoded.payload,
                "upstream",
            )
            .expect("encoded request");
        let body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
        assert_eq!(body["model"], "upstream");
        assert_eq!(body["future_field"]["enabled"], true);
        let debug = format!("{encoded:?}");
        assert!(!debug.contains("future_field"));
        assert!(!debug.contains("upstream"));
    }

    #[tokio::test]
    async fn compact_rejects_streaming_and_errors_use_openai_shape() {
        let adapter = OpenAiResponsesAdapter::new();
        assert!(
            adapter
                .decode_ingress_request(IngressRequest {
                    method: Method::POST,
                    uri: Uri::from_static("/v1/responses/compact"),
                    headers: HeaderMap::new(),
                    body: Bytes::from_static(br#"{"model":"public","stream":true}"#),
                    operation: ProtocolOperation::ResponsesCompact,
                })
                .await
                .is_err()
        );
        let response =
            adapter.error_response(&PublicError::new(PublicErrorCode::ModelNotFound, "missing"));
        let body: Value = serde_json::from_slice(&response.body).expect("error JSON");
        assert_eq!(response.status, http::StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "model_not_found");
        assert_eq!(body["error"]["param"], "model");

        let not_found = adapter.error_response(&PublicError::new(
            PublicErrorCode::PublicApiNotFound,
            "missing route",
        ));
        let body: Value = serde_json::from_slice(&not_found.body).expect("error JSON");
        assert_eq!(not_found.status, http::StatusCode::NOT_FOUND);
        assert_eq!(body["error"]["code"], "public_api_not_found");

        let method = adapter.error_response(&PublicError::new(
            PublicErrorCode::MethodNotAllowed,
            "wrong method",
        ));
        let body: Value = serde_json::from_slice(&method.body).expect("error JSON");
        assert_eq!(method.status, http::StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(body["error"]["code"], "method_not_allowed");
    }

    #[test]
    fn parsed_json_payload_debug_is_redacted() {
        let payload = AdapterPayload::Json(json!({}));
        assert!(matches!(payload, AdapterPayload::Json(_)));
        assert_eq!(format!("{payload:?}"), "Json([REDACTED])");
    }

    #[test]
    fn responses_stream_rewrites_the_public_model() {
        let adapter = OpenAiResponsesAdapter::new();
        let event = adapter
            .decode_upstream_event(SseFrame(Bytes::from_static(
                b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"model\":\"upstream\"}}\n\n",
            )))
            .expect("decoded event");
        let frame = adapter
            .encode_egress_event(event, "public")
            .expect("encoded event");
        assert!(String::from_utf8_lossy(&frame.0).contains(r#""model":"public""#));
    }

    #[test]
    fn extracts_continuation_id_from_a_json_response() {
        let adapter = OpenAiResponsesAdapter::new();
        let body = Bytes::from_static(br#"{"id":"resp_json","object":"response"}"#);
        let response = DecodedUpstreamResponse {
            status: http::StatusCode::OK,
            headers: HeaderMap::new(),
            parsed: serde_json::from_slice(&body).expect("response JSON"),
            body: Some(body),
            telemetry: Default::default(),
        };

        assert_eq!(
            adapter
                .continuation_id_from_response(ProtocolOperation::Responses, &response)
                .expect("response identity"),
            Some("resp_json".into())
        );
        assert_eq!(
            adapter
                .continuation_id_from_response(ProtocolOperation::ResponsesCompact, &response)
                .expect("compact has no continuation identity"),
            None
        );
    }

    #[test]
    fn extracts_continuation_id_from_response_created_sse() {
        let adapter = OpenAiResponsesAdapter::new();
        let event = adapter
            .decode_upstream_event(SseFrame(Bytes::from_static(
                b"event: response.created\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_sse\"}}\r\n\r\n",
            )))
            .expect("decoded event");

        assert_eq!(
            adapter
                .continuation_id_from_event(ProtocolOperation::Responses, &event)
                .expect("event identity"),
            Some("resp_sse".into())
        );
    }
}
