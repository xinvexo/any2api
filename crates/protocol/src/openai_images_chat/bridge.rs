use any2api_domain::{ProtocolDialect, ProtocolOperation};

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, DecodedRequest, DecodedUpstreamResponse, ProtocolBridge,
        ProtocolBridgeSession, StartedProtocolBridge,
    },
    json_codec,
};

use super::{request, response};

#[derive(Default)]
pub struct ImagesToChatCompletionsBridge;

impl ImagesToChatCompletionsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl ProtocolBridge for ImagesToChatCompletionsBridge {
    fn ingress_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiImages
    }

    fn upstream_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
    }

    fn supports_operation(&self, operation: ProtocolOperation) -> bool {
        operation == ProtocolOperation::ImagesGenerations
    }

    fn start(
        &self,
        decoded: &DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        if decoded.operation != ProtocolOperation::ImagesGenerations {
            return Err(ProtocolError::Unsupported(format!(
                "{:?}",
                decoded.operation
            )));
        }
        let value = decoded.payload.materialize_json().map_err(|_| {
            ProtocolError::InvalidPayload("Images bridge requires a JSON request body".into())
        })?;
        let converted = request::convert(value.as_ref(), upstream_model)?;
        let request = json_codec::encode_json_request(
            ProtocolOperation::ChatCompletions,
            &decoded.headers,
            &converted.body,
            upstream_model,
        )?;
        Ok(StartedProtocolBridge::new(
            ProtocolOperation::ChatCompletions,
            request,
            Box::new(ImagesToChatSession {
                expected_choices: converted.expected_choices,
            }),
        ))
    }
}

struct ImagesToChatSession {
    expected_choices: usize,
}

impl ProtocolBridgeSession for ImagesToChatSession {
    fn transform_response(
        &mut self,
        mut decoded: DecodedUpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        decoded.parsed = response::convert(decoded.parsed, self.expected_choices)?;
        decoded.body = None;
        Ok(decoded)
    }

    fn transform_event(
        &mut self,
        _event: AdapterEvent,
    ) -> Result<Vec<AdapterEvent>, ProtocolError> {
        Err(ProtocolError::InvalidPayload(
            "Images to Chat Completions bridge does not support streaming".into(),
        ))
    }
}

impl std::fmt::Debug for ImagesToChatCompletionsBridge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImagesToChatCompletionsBridge")
            .finish_non_exhaustive()
    }
}
