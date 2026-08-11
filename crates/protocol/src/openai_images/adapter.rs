use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, RequestBodyEncoding};
use async_trait::async_trait;
use http::{HeaderMap, HeaderValue, Method, Uri, header};

use crate::{
    ProtocolError, affinity,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedResponsePayload,
        DecodedUpstreamResponse, EgressResponse, EncodedUpstreamRequest, IngressRequest,
        ProtocolAdapter, RequestExecutionProfile, SseFrame, StreamCompletionPolicy,
        UpstreamResponse,
    },
    json_codec,
    sse::{parse_event_payload, rewrite_known_model},
};

use super::{multipart, telemetry, termination};

#[derive(Debug, Default)]
pub struct OpenAiImagesAdapter;

impl OpenAiImagesAdapter {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ProtocolAdapter for OpenAiImagesAdapter {
    fn dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiImages
    }

    fn stream_completion_policy(&self, operation: ProtocolOperation) -> StreamCompletionPolicy {
        if matches!(
            operation,
            ProtocolOperation::ImagesGenerations | ProtocolOperation::ImagesEdits
        ) {
            StreamCompletionPolicy::TerminalEventRequired
        } else {
            StreamCompletionPolicy::EofAllowed
        }
    }

    async fn decode_ingress_request(
        &self,
        request: IngressRequest,
    ) -> Result<DecodedRequest, ProtocolError> {
        if request.operation != ProtocolOperation::ImagesEdits
            || !is_multipart_content_type(&request.headers)
        {
            return json_codec::decode_request(request, self.dialect());
        }
        if request.method != Method::POST || request.operation.dialect() != self.dialect() {
            return Err(ProtocolError::Unsupported(format!(
                "{:?}",
                request.operation
            )));
        }
        let content_type = request
            .headers
            .get(header::CONTENT_TYPE)
            .ok_or_else(|| {
                ProtocolError::InvalidPayload("multipart content type is missing".into())
            })?
            .to_str()
            .map_err(|_| {
                ProtocolError::InvalidPayload("multipart content type is invalid".into())
            })?;
        let payload = multipart::parse(request.body, content_type).await?;
        let model = payload.model.clone();
        let affinity =
            affinity::extract(request.operation, &request.headers, &serde_json::Map::new())?;
        Ok(DecodedRequest {
            dialect: self.dialect(),
            operation: request.operation,
            execution_profile: RequestExecutionProfile::Standard,
            client_headers: request.headers,
            headers: HeaderMap::new(),
            body_encoding: RequestBodyEncoding::Identity,
            model: Some(model),
            stream: payload.stream,
            thinking_level: None,
            affinity,
            payload: AdapterPayload::Multipart(payload),
        })
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
            ProtocolOperation::ImagesGenerations | ProtocolOperation::ImagesEdits
        ) {
            return Err(ProtocolError::Unsupported(format!("{operation:?}")));
        }
        let payload = match payload {
            AdapterPayload::Json(_) | AdapterPayload::RawJson(_) => {
                return json_codec::encode_request(operation, headers, payload, upstream_model);
            }
            AdapterPayload::Multipart(payload) => payload,
        };
        if operation != ProtocolOperation::ImagesEdits {
            return Err(ProtocolError::InvalidPayload(
                "image generations requests must be JSON".into(),
            ));
        }
        let stream = payload.stream;
        let (body, content_type) = multipart::encode(payload, upstream_model)?;
        let mut headers = headers.clone();
        headers.insert(header::CONTENT_TYPE, content_type);
        headers.insert(
            header::ACCEPT,
            HeaderValue::from_static(if stream {
                "text/event-stream"
            } else {
                "application/json"
            }),
        );
        Ok(EncodedUpstreamRequest {
            method: Method::POST,
            uri: Uri::from_static("/"),
            headers,
            body,
        })
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
        let payload = parse_event_payload(&frame.0);
        let telemetry = telemetry::event(&payload);
        let termination = termination::classify(&payload);
        let rejection = crate::stream_rejection::openai(&payload);
        Ok(AdapterEvent::new(frame.0, telemetry, payload)
            .with_termination(termination)
            .with_rejection(rejection))
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
        crate::OpenAiResponsesAdapter::new().error_response(error)
    }
}

fn is_multipart_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("multipart/form-data"))
}
