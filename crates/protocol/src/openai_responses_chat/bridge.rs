use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, BridgeContinuationState, DecodedRequest, DecodedResponsePayload,
        DecodedUpstreamResponse, MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolBridge,
        ProtocolBridgeCapabilities, ProtocolBridgeSession, ProtocolContinuationState,
        ResumableProtocolContinuation, StartedProtocolBridge,
    },
    json_codec,
};

use super::{capabilities::CAPABILITIES, request, response, stream::ChatToResponsesStream};

#[derive(Default)]
pub struct ResponsesToChatCompletionsBridge;

impl ResponsesToChatCompletionsBridge {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ProtocolBridge for ResponsesToChatCompletionsBridge {
    fn ingress_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiResponses
    }

    fn upstream_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
    }

    fn capabilities(&self) -> &'static ProtocolBridgeCapabilities {
        &CAPABILITIES
    }

    fn start(
        &self,
        decoded: &DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        if decoded.operation != ProtocolOperation::Responses {
            return Err(ProtocolError::Unsupported(format!(
                "{:?}",
                decoded.operation
            )));
        }
        start_session(decoded, upstream_model, None)
    }
}

struct ResponsesToChatSession {
    conversation: Vec<Value>,
    stream: ChatToResponsesStream,
    response_id: String,
    continuation: Option<ProtocolContinuationState>,
    accumulated_stream_bytes: usize,
}

impl ResponsesToChatSession {
    fn complete(&mut self, assistant_message: Value) -> Result<(), ProtocolError> {
        if self.continuation.is_some() {
            return Ok(());
        }
        self.conversation.push(assistant_message);
        let continuation = ResponsesChatContinuation::new(std::mem::take(&mut self.conversation))?;
        self.continuation = Some(ProtocolContinuationState::new(Arc::new(continuation))?);
        Ok(())
    }

    fn observe_stream_event(&mut self, event: &AdapterEvent) -> Result<(), ProtocolError> {
        self.accumulated_stream_bytes = self
            .accumulated_stream_bytes
            .checked_add(event.bytes().len())
            .ok_or(ProtocolError::ContinuationTooLarge {
                bytes: usize::MAX,
                max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
            })?;
        if self.accumulated_stream_bytes > MAX_BRIDGE_CONTINUATION_STATE_BYTES {
            return Err(ProtocolError::ContinuationTooLarge {
                bytes: self.accumulated_stream_bytes,
                max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
            });
        }
        Ok(())
    }
}

impl ProtocolBridgeSession for ResponsesToChatSession {
    fn transform_response(
        &mut self,
        mut decoded: DecodedUpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        let DecodedResponsePayload::StructuredJson(parsed) = decoded.payload else {
            return Err(ProtocolError::InvalidPayload(
                "protocol bridge requires a structured response".into(),
            ));
        };
        let converted = response::convert(parsed, &self.response_id)?;
        self.complete(converted.assistant_message)?;
        decoded.payload = DecodedResponsePayload::StructuredJson(converted.body);
        Ok(decoded)
    }

    fn transform_event(&mut self, event: AdapterEvent) -> Result<Vec<AdapterEvent>, ProtocolError> {
        self.observe_stream_event(&event)?;
        let update = self.stream.push(event)?;
        if let Some(message) = update.assistant_message {
            self.complete(message)?;
        }
        Ok(update.events)
    }

    fn finish_events(&mut self) -> Result<Vec<AdapterEvent>, ProtocolError> {
        let update = self.stream.finish()?;
        if let Some(message) = update.assistant_message {
            self.complete(message)?;
        }
        Ok(update.events)
    }

    fn continuation_state(&self) -> BridgeContinuationState {
        self.continuation
            .as_ref()
            .map_or(BridgeContinuationState::Pending, |state| {
                BridgeContinuationState::Ready(state.clone())
            })
    }
}

struct ResponsesChatContinuation {
    conversation: Arc<[Value]>,
    serialized_bytes: usize,
}

impl ResponsesChatContinuation {
    fn new(conversation: Vec<Value>) -> Result<Self, ProtocolError> {
        let serialized_bytes = serialized_json_bytes(&conversation)?;
        if serialized_bytes > MAX_BRIDGE_CONTINUATION_STATE_BYTES {
            return Err(ProtocolError::ContinuationTooLarge {
                bytes: serialized_bytes,
                max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
            });
        }
        Ok(Self {
            conversation: conversation.into(),
            serialized_bytes,
        })
    }
}

impl ResumableProtocolContinuation for ResponsesChatContinuation {
    fn ingress_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiResponses
    }

    fn upstream_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
    }

    fn operation(&self) -> ProtocolOperation {
        ProtocolOperation::Responses
    }

    fn serialized_bytes(&self) -> usize {
        self.serialized_bytes
    }

    fn resume(
        &self,
        request: &DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        start_session(request, upstream_model, Some(self.conversation.as_ref()))
    }
}

fn start_session(
    decoded: &DecodedRequest,
    upstream_model: &str,
    previous: Option<&[Value]>,
) -> Result<StartedProtocolBridge, ProtocolError> {
    let value = decoded.payload.materialize_json().map_err(|_| {
        ProtocolError::InvalidPayload("Responses bridge requires a JSON request body".into())
    })?;
    validate_continuation_reference(value.as_ref(), previous.is_some())?;
    let converted = request::convert(
        value.as_ref(),
        upstream_model,
        previous.map_or_else(Vec::new, <[Value]>::to_vec),
    )?;
    let accumulated_stream_bytes = serialized_json_bytes(converted.conversation())?;
    if accumulated_stream_bytes > MAX_BRIDGE_CONTINUATION_STATE_BYTES {
        return Err(ProtocolError::ContinuationTooLarge {
            bytes: accumulated_stream_bytes,
            max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
        });
    }
    let request = json_codec::encode_json_request(
        ProtocolOperation::ChatCompletions,
        &decoded.headers,
        &converted.body,
        upstream_model,
    )?;
    let conversation = converted.into_conversation();
    let response_id = format!("resp_{}", Uuid::new_v4().simple());
    Ok(StartedProtocolBridge::new(
        ProtocolOperation::ChatCompletions,
        request,
        Box::new(ResponsesToChatSession {
            conversation,
            stream: ChatToResponsesStream::new(response_id.clone(), upstream_model.into()),
            response_id,
            continuation: None,
            accumulated_stream_bytes,
        }),
    ))
}

fn validate_continuation_reference(
    value: &Value,
    continuation_supplied: bool,
) -> Result<(), ProtocolError> {
    match value.get("previous_response_id") {
        Some(Value::String(id)) if !id.is_empty() && continuation_supplied => Ok(()),
        Some(Value::String(id)) if !id.is_empty() => Err(ProtocolError::SessionBindingLost),
        Some(Value::Null) | None if !continuation_supplied => Ok(()),
        Some(Value::Null) | None => Err(ProtocolError::SessionBindingLost),
        Some(_) => Err(ProtocolError::InvalidPayload(
            "previous_response_id must be a string".into(),
        )),
    }
}

fn serialized_json_bytes(value: &[Value]) -> Result<usize, ProtocolError> {
    struct CountingWriter(usize);

    impl std::io::Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(buffer.len());
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = CountingWriter(0);
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        ProtocolError::InvalidPayload("continuation state is not serializable".into())
    })?;
    Ok(writer.0)
}

impl std::fmt::Debug for ResponsesToChatCompletionsBridge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResponsesToChatCompletionsBridge")
            .finish_non_exhaustive()
    }
}
