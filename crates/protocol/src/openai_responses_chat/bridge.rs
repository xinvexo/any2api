use std::sync::Arc;

use any2api_domain::{OpenAiChatCompletionsProfile, ProtocolDialect, ProtocolOperation};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, BridgeContinuationState, DecodedRequest, DecodedResponsePayload,
        DecodedUpstreamResponse, MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolBridge,
        ProtocolBridgeCapabilities, ProtocolBridgeContext, ProtocolBridgeSession,
        ProtocolContinuationState, StartedProtocolBridge,
    },
    json_codec,
};

use super::{
    capabilities::CAPABILITIES,
    continuation::{ResponsesChatContinuation, serialized_json_bytes},
    request, response,
    response_projection::ResponseProjection,
    stream::ChatToResponsesStream,
    tool_projection::ToolProjection,
};

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
        context: ProtocolBridgeContext<'_>,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        if decoded.operation != ProtocolOperation::Responses {
            return Err(ProtocolError::Unsupported(format!(
                "{:?}",
                decoded.operation
            )));
        }
        let profile = context
            .target_profile
            .openai_chat_completions()
            .ok_or_else(|| ProtocolError::Unsupported("Chat target profile is required".into()))?;
        start_session(decoded, context.upstream_model, profile, None)
    }
}

struct ResponsesToChatSession {
    conversation: Vec<Value>,
    stream: ChatToResponsesStream,
    response_projection: ResponseProjection,
    continuation: Option<ProtocolContinuationState>,
    accumulated_stream_bytes: usize,
    profile: OpenAiChatCompletionsProfile,
    projection: ToolProjection,
}

impl ResponsesToChatSession {
    fn complete(&mut self, assistant_message: Value) -> Result<(), ProtocolError> {
        if self.continuation.is_some() {
            return Ok(());
        }
        self.conversation.push(assistant_message);
        let continuation = ResponsesChatContinuation::new(
            std::mem::take(&mut self.conversation),
            self.projection.clone(),
        )?;
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
        let converted = response::convert(
            parsed,
            &self.response_projection,
            &self.projection,
            self.profile,
        )?;
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

pub(super) fn start_session(
    decoded: &DecodedRequest,
    upstream_model: &str,
    profile: OpenAiChatCompletionsProfile,
    previous: Option<&ResponsesChatContinuation>,
) -> Result<StartedProtocolBridge, ProtocolError> {
    let value = decoded.payload.materialize_json().map_err(|_| {
        ProtocolError::InvalidPayload("Responses bridge requires a JSON request body".into())
    })?;
    validate_continuation_reference(value.as_ref(), previous.is_some())?;
    let converted = request::convert(
        value.as_ref(),
        upstream_model,
        profile,
        previous.map_or_else(Vec::new, |state| state.conversation().to_vec()),
        previous.map(ResponsesChatContinuation::projection),
    )?;
    let accumulated_stream_bytes = serialized_json_bytes(converted.conversation())?
        .checked_add(converted.projection().serialized_bytes())
        .ok_or(ProtocolError::ContinuationTooLarge {
            bytes: usize::MAX,
            max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
        })?;
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
    let projection = converted.projection().clone();
    let conversation = converted.into_conversation();
    let response_projection = ResponseProjection::new(
        format!("resp_{}", Uuid::new_v4().simple()),
        upstream_model.to_owned(),
        value.as_ref(),
    );
    Ok(StartedProtocolBridge::new(
        ProtocolOperation::ChatCompletions,
        request,
        Box::new(ResponsesToChatSession {
            conversation,
            stream: ChatToResponsesStream::new(
                response_projection.clone(),
                profile,
                projection.clone(),
            ),
            response_projection,
            continuation: None,
            accumulated_stream_bytes,
            profile,
            projection,
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

impl std::fmt::Debug for ResponsesToChatCompletionsBridge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResponsesToChatCompletionsBridge")
            .finish_non_exhaustive()
    }
}
