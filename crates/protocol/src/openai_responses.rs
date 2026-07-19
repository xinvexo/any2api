use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, PublicErrorCode};
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use serde_json::{Value, json};

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse,
    },
    json_codec,
    sse::rewrite_known_model,
};

#[derive(Debug, Default)]
pub struct OpenAiResponsesAdapter;

impl OpenAiResponsesAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProtocolAdapter for OpenAiResponsesAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiResponses
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
        Ok(DecodedUpstreamResponse {
            status: response.status,
            headers: response.headers,
            payload: AdapterPayload::RawJson(response.body),
        })
    }

    fn decode_upstream_event(&self, frame: SseFrame) -> Result<AdapterEvent, ProtocolError> {
        Ok(AdapterEvent(frame.0))
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
        rewrite_known_model(SseFrame(event.0), public_model)
    }

    fn hard_affinity_id_from_response(
        &self,
        operation: ProtocolOperation,
        response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        if operation != ProtocolOperation::Responses {
            return Ok(None);
        }
        let AdapterPayload::RawJson(body) = &response.payload;
        let value: Value = serde_json::from_slice(body).map_err(|_| {
            ProtocolError::InvalidPayload("response body must be valid JSON".into())
        })?;
        optional_non_empty_id(value.get("id"), "response id")
    }

    fn hard_affinity_id_from_event(
        &self,
        operation: ProtocolOperation,
        event: &AdapterEvent,
    ) -> Result<Option<String>, ProtocolError> {
        if operation != ProtocolOperation::Responses {
            return Ok(None);
        }
        let Some((event_name, value)) = sse_json_event(&event.0)? else {
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
        let code = error_code(error.code);
        let error_type = error_type(error.code);
        json_response(
            public_error_status(error.code),
            json!({
                "error": {
                    "message": error.message,
                    "type": error_type,
                    "param": null,
                    "code": code
                }
            }),
        )
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

fn sse_json_event(bytes: &[u8]) -> Result<Option<(Option<String>, Value)>, ProtocolError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ProtocolError::InvalidPayload("SSE frame is not valid UTF-8".into()))?;
    let normalized = text.replace("\r\n", "\n");
    let event = normalized
        .lines()
        .find_map(|line| line.strip_prefix("event:"))
        .map(str::trim)
        .map(str::to_owned);
    let data = normalized
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data.trim() == "[DONE]" {
        return Ok(None);
    }
    let value = serde_json::from_str(&data)
        .map_err(|_| ProtocolError::InvalidPayload("SSE data is not valid JSON".into()))?;
    Ok(Some((event, value)))
}

fn error_type(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "authentication_error",
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::ModelNotFound
        | PublicErrorCode::NoRoute
        | PublicErrorCode::UpstreamNotFound
        | PublicErrorCode::SessionBindingLost => "invalid_request_error",
        PublicErrorCode::NoAvailableCredential | PublicErrorCode::LocalConcurrencyLimit => {
            "rate_limit_error"
        }
        PublicErrorCode::UpstreamError | PublicErrorCode::InternalError => "server_error",
    }
}

fn error_code(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "unauthorized",
        PublicErrorCode::InvalidRequest => "invalid_request",
        PublicErrorCode::ModelNotFound | PublicErrorCode::NoRoute => "model_not_found",
        PublicErrorCode::UpstreamNotFound => "upstream_not_found",
        PublicErrorCode::NoAvailableCredential => "no_available_credential",
        PublicErrorCode::LocalConcurrencyLimit => "local_concurrency_limit",
        PublicErrorCode::SessionBindingLost => "session_binding_lost",
        PublicErrorCode::UpstreamError => "upstream_error",
        PublicErrorCode::InternalError => "internal_error",
    }
}

fn public_error_status(code: PublicErrorCode) -> StatusCode {
    match code {
        PublicErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        PublicErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
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

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode};
    use bytes::Bytes;
    use http::{HeaderMap, Method, Uri};
    use serde_json::{Value, json};

    use super::OpenAiResponsesAdapter;
    use crate::api::{
        AdapterEvent, AdapterPayload, DecodedUpstreamResponse, IngressRequest, ProtocolAdapter,
        SseFrame,
    };

    #[test]
    fn preserves_unknown_fields_and_rewrites_the_upstream_model() {
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
            .expect("decoded request");
        assert_eq!(decoded.model.as_deref(), Some("public"));
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                decoded.headers,
                decoded.payload,
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

    #[test]
    fn compact_rejects_streaming_and_errors_use_openai_shape() {
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
                .is_err()
        );
        let response =
            adapter.error_response(&PublicError::new(PublicErrorCode::ModelNotFound, "missing"));
        let body: Value = serde_json::from_slice(&response.body).expect("error JSON");
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert_eq!(body["error"]["code"], "model_not_found");
    }

    #[test]
    fn raw_json_payload_is_the_only_first_release_payload() {
        let payload = AdapterPayload::RawJson(Bytes::from_static(b"{}"));
        assert!(matches!(payload, AdapterPayload::RawJson(_)));
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
    fn extracts_hard_affinity_from_a_json_response() {
        let adapter = OpenAiResponsesAdapter::new();
        let response = DecodedUpstreamResponse {
            status: http::StatusCode::OK,
            headers: HeaderMap::new(),
            payload: AdapterPayload::RawJson(Bytes::from_static(
                br#"{"id":"resp_json","object":"response"}"#,
            )),
        };

        assert_eq!(
            adapter
                .hard_affinity_id_from_response(ProtocolOperation::Responses, &response)
                .expect("response identity"),
            Some("resp_json".into())
        );
        assert_eq!(
            adapter
                .hard_affinity_id_from_response(ProtocolOperation::ResponsesCompact, &response)
                .expect("compact has no hard identity"),
            None
        );
    }

    #[test]
    fn extracts_hard_affinity_from_response_created_sse() {
        let adapter = OpenAiResponsesAdapter::new();
        let event = AdapterEvent(Bytes::from_static(
            b"event: response.created\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_sse\"}}\r\n\r\n",
        ));

        assert_eq!(
            adapter
                .hard_affinity_id_from_event(ProtocolOperation::Responses, &event)
                .expect("event identity"),
            Some("resp_sse".into())
        );
    }
}
