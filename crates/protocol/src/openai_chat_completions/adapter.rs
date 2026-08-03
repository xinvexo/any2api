use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError};
use async_trait::async_trait;
use http::HeaderMap;

use crate::{
    OpenAiResponsesAdapter, ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
        EncodedUpstreamRequest, IngressRequest, ProtocolAdapter, SseFrame, StreamCompletionPolicy,
        UpstreamResponse,
    },
    json_codec,
    sse::{parse_event_payload, rewrite_known_model},
};

use super::{telemetry, termination};

#[derive(Debug, Default)]
pub struct OpenAiChatCompletionsAdapter;

impl OpenAiChatCompletionsAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolAdapter for OpenAiChatCompletionsAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
    }

    fn stream_completion_policy(&self, operation: ProtocolOperation) -> StreamCompletionPolicy {
        if operation == ProtocolOperation::ChatCompletions {
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
        if operation != ProtocolOperation::ChatCompletions {
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
    use crate::api::{
        IngressRequest, ProtocolAdapter, SseFrame, StreamCompletionPolicy, StreamTermination,
    };

    #[tokio::test]
    async fn decodes_and_rewrites_chat_completions() {
        let adapter = OpenAiChatCompletionsAdapter::new();
        let decoded = adapter
            .decode_ingress_request(IngressRequest {
                method: Method::POST,
                uri: Uri::from_static("/v1/chat/completions"),
                headers: HeaderMap::new(),
                body: Bytes::from_static(br#"{"model":"public","messages":[]}"#),
                operation: ProtocolOperation::ChatCompletions,
            })
            .await
            .expect("request");
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                &decoded.headers,
                &decoded.payload,
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

    #[test]
    fn chat_stream_distinguishes_done_from_empty_heartbeats_and_errors() {
        let adapter = OpenAiChatCompletionsAdapter::new();
        assert_eq!(
            adapter.stream_completion_policy(ProtocolOperation::ChatCompletions),
            StreamCompletionPolicy::TerminalEventRequired
        );
        assert_eq!(
            adapter
                .decode_upstream_event(SseFrame(Bytes::from_static(b"data: [DONE]\n\n")))
                .expect("done sentinel")
                .termination(),
            StreamTermination::Completed
        );
        assert_eq!(
            adapter
                .decode_upstream_event(SseFrame(Bytes::from_static(b": keep-alive\n\n")))
                .expect("empty heartbeat")
                .termination(),
            StreamTermination::None
        );
        assert_eq!(
            adapter
                .decode_upstream_event(SseFrame(Bytes::from_static(
                    b"event: error\ndata: {\"error\":{\"message\":\"busy\"}}\n\n",
                )))
                .expect("error event")
                .termination(),
            StreamTermination::Failed
        );
    }
}
