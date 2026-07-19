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

    fn decode_upstream_event(&self, _frame: SseFrame) -> Result<AdapterEvent, ProtocolError> {
        Err(ProtocolError::Unsupported("responses SSE".into()))
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

    fn encode_egress_event(&self, _event: AdapterEvent) -> Result<SseFrame, ProtocolError> {
        Err(ProtocolError::Unsupported("responses SSE".into()))
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

fn error_type(code: PublicErrorCode) -> &'static str {
    match code {
        PublicErrorCode::Unauthorized => "authentication_error",
        PublicErrorCode::InvalidRequest
        | PublicErrorCode::ModelNotFound
        | PublicErrorCode::NoRoute
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
        PublicErrorCode::ModelNotFound | PublicErrorCode::NoRoute => StatusCode::NOT_FOUND,
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
    use crate::api::{AdapterPayload, IngressRequest, ProtocolAdapter};

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
}
