mod wire;

use std::collections::HashSet;

use any2api_domain::OpenAiChatReasoningResponse;
use serde_json::{Value, json};

use super::{
    super::wire::{SynthesizedEvent, content_telemetry, event, event_default},
    ChatToResponsesStream,
};
use crate::{
    ProtocolError,
    openai_responses_chat::{
        response::restored_call_item,
        tool_projection::{ChatToolKind, RestoredToolCall, ToolIdentity},
    },
};

use self::wire::{
    added_event, append_payload, emit_payload_delta, item_id, observe_kind, payload_object,
    set_once,
};

#[derive(Default)]
pub(super) struct ToolState {
    output_index: Option<usize>,
    item_id: String,
    kind: Option<ChatToolKind>,
    call_id: Option<String>,
    name: Option<String>,
    payload: String,
    emitted_payload: usize,
    identity: Option<ToolIdentity>,
    done: bool,
}

impl ChatToResponsesStream {
    pub(super) fn push_tool(
        &mut self,
        call: &Value,
    ) -> Result<Vec<SynthesizedEvent>, ProtocolError> {
        let object = call
            .as_object()
            .ok_or_else(|| invalid("streamed tool call must be an object"))?;
        let key = object
            .get("index")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("streamed tool call index must be a non-negative integer"))?;
        let state = self.tools.entry(key).or_default();
        observe_kind(state, object)?;
        set_once(
            &mut state.call_id,
            object.get("id"),
            "streamed tool call id",
        )?;
        let payload = payload_object(state.kind, object)?;
        if let Some(payload) = payload {
            set_once(
                &mut state.name,
                payload.get("name"),
                "streamed tool call name",
            )?;
            append_payload(state, payload)?;
        }
        if state.identity.is_none()
            && let (Some(kind), Some(name)) = (state.kind, state.name.as_deref())
        {
            state.identity = Some(self.projection.active_identity_for_call(kind, name)?);
        }

        let streamable = state
            .call_id
            .as_deref()
            .is_some_and(|value| !value.is_empty())
            && state.identity.as_ref().is_some_and(|identity| {
                matches!(
                    identity,
                    ToolIdentity::Function { .. } | ToolIdentity::NamespaceFunction { .. }
                ) || matches!(
                    identity,
                    ToolIdentity::Custom { .. } | ToolIdentity::NamespaceCustom { .. }
                ) && state.kind == Some(ChatToolKind::Custom)
            });
        let mut events = Vec::new();
        if streamable && state.output_index.is_none() {
            let index = self.next_output_index;
            self.next_output_index += 1;
            state.output_index = Some(index);
            state.item_id = item_id(self.response.response_id(), key, state.identity.as_ref())?;
            events.push(added_event(index, state)?);
        }
        events.extend(emit_payload_delta(state));
        Ok(events)
    }

    pub(super) fn finish_tools(&mut self) -> Result<Vec<SynthesizedEvent>, ProtocolError> {
        let keys = self.tools.keys().copied().collect::<Vec<_>>();
        let mut events = Vec::new();
        for key in keys {
            let mut state = self
                .tools
                .remove(&key)
                .ok_or_else(|| invalid("streamed tool state disappeared"))?;
            events.extend(self.finish_tool(key, &mut state)?);
            self.tools.insert(key, state);
        }
        Ok(events)
    }

    pub(super) fn validate_tool_identities(&self) -> Result<(), ProtocolError> {
        let mut call_ids = HashSet::new();
        for state in self.tools.values() {
            let kind = state.kind.ok_or_else(|| {
                invalid("streamed tool call identity is incomplete: type is missing")
            })?;
            let call_id = state
                .call_id
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    invalid("streamed tool call identity is incomplete: id is missing")
                })?;
            let name = state
                .name
                .as_deref()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    invalid("streamed tool call identity is incomplete: name is missing")
                })?;
            if !call_ids.insert(call_id) {
                return Err(invalid("streamed tool call ids must be unique"));
            }
            match kind {
                ChatToolKind::Function => {
                    self.projection
                        .restore_function_call(name, &state.payload)?;
                }
                ChatToolKind::Custom => {
                    self.projection.restore_custom_call(name, &state.payload)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn assistant_message(&self) -> Value {
        let mut message = json!({"role":"assistant","content": if self.message.text.is_empty() {
            Value::Null
        } else {
            Value::String(self.message.text.clone())
        }});
        if !self.reasoning.text.is_empty() {
            let field = match self.profile.reasoning_response {
                OpenAiChatReasoningResponse::Reasoning => "reasoning",
                _ => "reasoning_content",
            };
            message[field] = Value::String(self.reasoning.text.clone());
        }
        let calls = self
            .tools
            .values()
            .filter_map(ToolState::assistant_call)
            .collect::<Vec<_>>();
        if !calls.is_empty() {
            message["tool_calls"] = Value::Array(calls);
        }
        message
    }

    fn finish_tool(
        &mut self,
        key: u64,
        state: &mut ToolState,
    ) -> Result<Vec<SynthesizedEvent>, ProtocolError> {
        if state.done {
            return Ok(Vec::new());
        }
        let kind = state
            .kind
            .ok_or_else(|| invalid("streamed tool call type is incomplete"))?;
        let call_id = state
            .call_id
            .as_deref()
            .ok_or_else(|| invalid("streamed tool call id is incomplete"))?
            .to_owned();
        let name = state
            .name
            .as_deref()
            .ok_or_else(|| invalid("streamed tool call name is incomplete"))?;
        let restored = match kind {
            ChatToolKind::Function => self
                .projection
                .restore_function_call(name, &state.payload)?,
            ChatToolKind::Custom => self.projection.restore_custom_call(name, &state.payload)?,
        };
        let mut events = Vec::new();
        if state.output_index.is_none() {
            let index = self.next_output_index;
            self.next_output_index += 1;
            state.output_index = Some(index);
            state.item_id = item_id(self.response.response_id(), key, state.identity.as_ref())?;
            events.push(added_event(index, state)?);
        }
        events.extend(emit_payload_delta(state));
        let index = state
            .output_index
            .ok_or_else(|| invalid("streamed tool output index is missing"))?;
        if let RestoredToolCall::Custom { input, .. } = &restored
            && kind == ChatToolKind::Function
            && !input.is_empty()
        {
            events.push(event(
                "response.custom_tool_call_input.delta",
                json!({"type":"response.custom_tool_call_input.delta","item_id":state.item_id,
                    "output_index":index,"delta":input}),
                content_telemetry(),
            ));
        }
        match &restored {
            RestoredToolCall::Function { arguments, .. } => events.push(event_default(
                "response.function_call_arguments.done",
                json!({"type":"response.function_call_arguments.done","item_id":state.item_id,
                    "output_index":index,"arguments":arguments}),
            )),
            RestoredToolCall::Custom { input, .. } => events.push(event_default(
                "response.custom_tool_call_input.done",
                json!({"type":"response.custom_tool_call_input.done","item_id":state.item_id,
                    "output_index":index,"input":input}),
            )),
            RestoredToolCall::ToolSearch { .. } => {}
        }
        let item = restored_call_item(
            self.response.response_id(),
            key,
            &call_id,
            &restored,
            "completed",
        );
        self.completed_items.push((index, item.clone()));
        events.push(event_default(
            "response.output_item.done",
            json!({"type":"response.output_item.done","output_index":index,"item":item}),
        ));
        state.done = true;
        Ok(events)
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
