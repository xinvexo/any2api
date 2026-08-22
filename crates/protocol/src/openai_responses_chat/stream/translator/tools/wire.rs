use serde_json::{Map, Value, json};

use super::ToolState;
use crate::{
    ProtocolError,
    openai_responses_chat::{
        response::{
            custom_call_item_id, function_call_item_id, restored_call_item,
            tool_search_call_item_id,
        },
        stream::wire::{SynthesizedEvent, content_telemetry, event, event_default},
        tool_projection::{ChatToolKind, RestoredToolCall, ToolIdentity},
    },
};

impl ToolState {
    pub(super) fn assistant_call(&self) -> Option<Value> {
        let kind = self.kind?;
        let call_id = self.call_id.as_ref()?;
        let name = self.name.as_ref()?;
        Some(match kind {
            ChatToolKind::Function => json!({"id":call_id,"type":"function",
                "function":{"name":name,"arguments":self.payload}}),
            ChatToolKind::Custom => json!({"id":call_id,"type":"custom",
                "custom":{"name":name,"input":self.payload}}),
        })
    }
}

pub(super) fn observe_kind(
    state: &mut ToolState,
    object: &Map<String, Value>,
) -> Result<(), ProtocolError> {
    let explicit = match object.get("type") {
        None | Some(Value::Null) => None,
        Some(Value::String(kind)) if kind == "function" => Some(ChatToolKind::Function),
        Some(Value::String(kind)) if kind == "custom" => Some(ChatToolKind::Custom),
        Some(Value::String(kind)) => {
            return Err(ProtocolError::unsupported_value(
                "streamed tool call type",
                kind,
            ));
        }
        Some(_) => return Err(invalid("streamed tool call type must be a string")),
    };
    let inferred = match (
        object.get("function").is_some_and(|value| !value.is_null()),
        object.get("custom").is_some_and(|value| !value.is_null()),
    ) {
        (true, false) => Some(ChatToolKind::Function),
        (false, true) => Some(ChatToolKind::Custom),
        (true, true) => return Err(invalid("streamed tool call has two payload types")),
        (false, false) => None,
    };
    let observed = explicit.or(inferred);
    if explicit.is_some() && inferred.is_some() && explicit != inferred {
        return Err(invalid(
            "streamed tool call type conflicts with its payload",
        ));
    }
    if let Some(observed) = observed {
        match state.kind {
            None => state.kind = Some(observed),
            Some(existing) if existing == observed => {}
            Some(_) => return Err(invalid("streamed tool call type changed")),
        }
    }
    Ok(())
}

pub(super) fn payload_object(
    kind: Option<ChatToolKind>,
    object: &Map<String, Value>,
) -> Result<Option<&Map<String, Value>>, ProtocolError> {
    let field = match kind {
        Some(ChatToolKind::Function) => "function",
        Some(ChatToolKind::Custom) => "custom",
        None => return Ok(None),
    };
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(value)) => Ok(Some(value)),
        Some(_) => Err(invalid("streamed tool call payload must be an object")),
    }
}

pub(super) fn set_once(
    target: &mut Option<String>,
    value: Option<&Value>,
    field: &'static str,
) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        return Ok(());
    };
    let value = value.as_str().ok_or_else(|| invalid(field))?;
    match target {
        None => *target = Some(value.to_owned()),
        Some(existing) if existing == value => {}
        Some(_) => return Err(ProtocolError::InvalidPayload(format!("{field} changed"))),
    }
    Ok(())
}

pub(super) fn append_payload(
    state: &mut ToolState,
    payload: &Map<String, Value>,
) -> Result<(), ProtocolError> {
    let field = match state.kind {
        Some(ChatToolKind::Function) => "arguments",
        Some(ChatToolKind::Custom) => "input",
        None => return Ok(()),
    };
    if let Some(value) = payload.get(field) {
        let value = value
            .as_str()
            .ok_or_else(|| invalid("streamed tool call payload fragment must be a string"))?;
        state.payload.push_str(value);
    }
    Ok(())
}

pub(super) fn item_id(
    response_id: &str,
    key: u64,
    identity: Option<&ToolIdentity>,
) -> Result<String, ProtocolError> {
    match identity.ok_or_else(|| invalid("streamed tool projection is incomplete"))? {
        ToolIdentity::Function { .. } | ToolIdentity::NamespaceFunction { .. } => {
            Ok(function_call_item_id(response_id, key))
        }
        ToolIdentity::Custom { .. } | ToolIdentity::NamespaceCustom { .. } => {
            Ok(custom_call_item_id(response_id, key))
        }
        ToolIdentity::ToolSearch => Ok(tool_search_call_item_id(response_id, key)),
    }
}

pub(super) fn added_event(
    index: usize,
    state: &ToolState,
) -> Result<SynthesizedEvent, ProtocolError> {
    let identity = state
        .identity
        .as_ref()
        .ok_or_else(|| invalid("streamed tool projection is incomplete"))?;
    let call_id = state
        .call_id
        .as_deref()
        .ok_or_else(|| invalid("streamed tool call id is incomplete"))?;
    let call = initial_call(identity);
    let mut item = restored_call_item("placeholder", 0, call_id, &call, "in_progress");
    item["id"] = Value::String(state.item_id.clone());
    Ok(event_default(
        "response.output_item.added",
        json!({"type":"response.output_item.added","output_index":index,"item":item}),
    ))
}

pub(super) fn emit_payload_delta(state: &mut ToolState) -> Vec<SynthesizedEvent> {
    let Some(index) = state.output_index else {
        return Vec::new();
    };
    if state.payload.len() <= state.emitted_payload {
        return Vec::new();
    }
    let delta = state.payload[state.emitted_payload..].to_owned();
    state.emitted_payload = state.payload.len();
    let (kind, data) = match state.identity.as_ref() {
        Some(ToolIdentity::Function { .. } | ToolIdentity::NamespaceFunction { .. }) => (
            "response.function_call_arguments.delta",
            json!({"type":"response.function_call_arguments.delta","item_id":state.item_id,
                "output_index":index,"delta":delta}),
        ),
        Some(ToolIdentity::Custom { .. } | ToolIdentity::NamespaceCustom { .. })
            if state.kind == Some(ChatToolKind::Custom) =>
        {
            (
                "response.custom_tool_call_input.delta",
                json!({"type":"response.custom_tool_call_input.delta","item_id":state.item_id,
                    "output_index":index,"delta":delta}),
            )
        }
        _ => return Vec::new(),
    };
    vec![event(kind, data, content_telemetry())]
}

fn initial_call(identity: &ToolIdentity) -> RestoredToolCall {
    match identity {
        ToolIdentity::Function { name } => RestoredToolCall::Function {
            name: name.clone(),
            namespace: None,
            arguments: String::new(),
        },
        ToolIdentity::NamespaceFunction { namespace, name } => RestoredToolCall::Function {
            name: name.clone(),
            namespace: Some(namespace.clone()),
            arguments: String::new(),
        },
        ToolIdentity::Custom { name } => RestoredToolCall::Custom {
            name: name.clone(),
            namespace: None,
            input: String::new(),
        },
        ToolIdentity::NamespaceCustom { namespace, name } => RestoredToolCall::Custom {
            name: name.clone(),
            namespace: Some(namespace.clone()),
            input: String::new(),
        },
        ToolIdentity::ToolSearch => RestoredToolCall::ToolSearch {
            arguments: json!({}),
        },
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
