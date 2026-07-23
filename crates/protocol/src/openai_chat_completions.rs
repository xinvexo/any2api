use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError};
use http::HeaderMap;

use crate::{
    OpenAiResponsesAdapter, ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse,
    },
    json_codec,
    sse::rewrite_known_model,
};

mod telemetry;

#[derive(Debug, Default)]
pub struct OpenAiChatCompletionsAdapter;

impl OpenAiChatCompletionsAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProtocolAdapter for OpenAiChatCompletionsAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
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
        if operation != ProtocolOperation::ChatCompletions {
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
            payload: AdapterPayload::RawJson(response.body.clone()),
            telemetry: telemetry::response(&response.body),
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
        OpenAiResponsesAdapter::new().error_response(error)
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use bytes::Bytes;
    use http::{HeaderMap, Method, Uri};
    use serde_json::Value;

    use super::OpenAiChatCompletionsAdapter;
    use crate::api::{IngressRequest, ProtocolAdapter, SseFrame};

    #[test]
    fn decodes_and_rewrites_chat_completions() {
        let adapter = OpenAiChatCompletionsAdapter::new();
        let decoded = adapter
            .decode_ingress_request(IngressRequest {
                method: Method::POST,
                uri: Uri::from_static("/v1/chat/completions"),
                headers: HeaderMap::new(),
                body: Bytes::from_static(br#"{"model":"public","messages":[]}"#),
                operation: ProtocolOperation::ChatCompletions,
            })
            .expect("request");
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                decoded.headers,
                decoded.payload,
                "upstream",
            )
            .expect("upstream request");
        let body: Value = serde_json::from_slice(&encoded.body).expect("JSON");
        assert_eq!(body["model"], "upstream");
    }

    #[test]
    fn preserves_chat_sse_frames() {
        let adapter = OpenAiChatCompletionsAdapter::new();
        let event = adapter
            .decode_upstream_event(SseFrame(Bytes::from_static(
                b"data: {\"id\":\"x\",\"model\":\"upstream\",\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            )))
            .expect("event");
        let frame = adapter.encode_egress_event(event, "public").expect("frame");
        assert!(String::from_utf8_lossy(&frame.0).contains("\"model\":\"public\""));
    }
}
