mod items;
mod metadata;
mod usage;

use std::collections::HashSet;

use any2api_domain::{OpenAiChatCompletionsProfile, OpenAiChatReasoningResponse};
use serde_json::{Map, Value, json};

use crate::ProtocolError;

use super::{
    response_projection::ResponseProjection,
    tool_projection::{RestoredToolCall, ToolProjection},
};

pub(super) use items::{
    custom_call_item_id, function_call_item_id, message_item, message_item_id, reasoning_item,
    reasoning_item_id, restored_call_item, tool_search_call_item_id,
};
pub(super) use usage::{responses_usage, token_usage};

pub(super) struct ConvertedResponse {
    pub(super) body: Value,
    pub(super) assistant_message: Value,
}

pub(super) fn convert(
    body: Value,
    projection: &ResponseProjection,
    tools: &ToolProjection,
    profile: OpenAiChatCompletionsProfile,
) -> Result<ConvertedResponse, ProtocolError> {
    let choices = body
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Chat Completions response has no choices"))?;
    if choices.len() != 1 {
        return Err(invalid(
            "Chat Completions response must contain exactly one choice",
        ));
    }
    let choice = choices[0]
        .as_object()
        .ok_or_else(|| invalid("Chat Completions choice must be an object"))?;
    if choice.get("index").and_then(Value::as_u64) != Some(0) {
        return Err(invalid("Chat Completions choice index must be 0"));
    }
    let finish_reason = finish_reason(choice.get("finish_reason"))?;
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Chat Completions choice has no message"))?;
    validate_message(message)?;

    let mut output = Vec::new();
    if let Some(reasoning) = reasoning_text(message, profile)?
        && !reasoning.is_empty()
    {
        output.push(reasoning_item(projection.response_id(), &reasoning));
    }
    if let Some(text) = message_text(message)?
        && !text.is_empty()
    {
        output.push(message_item(projection.response_id(), &text));
    }
    append_tool_calls(message, projection.response_id(), tools, &mut output)?;
    if finish_reason == "tool_calls"
        && message
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        return Err(invalid(
            "finish_reason tool_calls requires at least one tool call",
        ));
    }
    if finish_reason != "tool_calls"
        && message
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| !calls.is_empty())
    {
        return Err(invalid(
            "Chat Completions tool calls require finish_reason tool_calls",
        ));
    }

    let incomplete_reason = incomplete_reason(Some(finish_reason));
    let status = if incomplete_reason.is_some() {
        "incomplete"
    } else {
        "completed"
    };
    let created_at = metadata::created_at(body.get("created"), projection.created_at())?;
    let model = metadata::response_model(body.get("model"), projection.upstream_model())?;
    let usage = responses_usage(body.get("usage"), profile)?;
    let mut response = projection.base_response(status, output, created_at, &model, usage);
    if let Some(reason) = incomplete_reason {
        response["incomplete_details"] = json!({"reason":reason});
    }

    Ok(ConvertedResponse {
        body: response,
        assistant_message: Value::Object(message.clone()),
    })
}

pub(super) fn incomplete_reason(finish_reason: Option<&str>) -> Option<&'static str> {
    match finish_reason {
        Some("length") => Some("max_output_tokens"),
        Some("content_filter") => Some("content_filter"),
        _ => None,
    }
}

pub(super) fn validate_finish_reason(value: &str) -> Result<(), ProtocolError> {
    if matches!(value, "stop" | "length" | "tool_calls" | "content_filter") {
        Ok(())
    } else {
        Err(ProtocolError::unsupported_value("finish_reason", value))
    }
}

fn finish_reason(value: Option<&Value>) -> Result<&str, ProtocolError> {
    let value = value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Chat Completions finish_reason must be a string"))?;
    validate_finish_reason(value)?;
    Ok(value)
}

fn validate_message(message: &Map<String, Value>) -> Result<(), ProtocolError> {
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(invalid(
            "Chat Completions response message role must be assistant",
        ));
    }
    for field in ["function_call", "refusal", "audio"] {
        if message.get(field).is_some_and(|value| !value.is_null()) {
            return Err(invalid(
                "Chat Completions response contains an unsupported output",
            ));
        }
    }
    Ok(())
}

fn append_tool_calls(
    message: &Map<String, Value>,
    response_id: &str,
    projection: &ToolProjection,
    output: &mut Vec<Value>,
) -> Result<(), ProtocolError> {
    let Some(value) = message.get("tool_calls") else {
        return Ok(());
    };
    let calls = value
        .as_array()
        .ok_or_else(|| invalid("Chat Completions tool_calls must be an array"))?;
    let mut seen_ids = HashSet::new();
    for (index, call) in calls.iter().enumerate() {
        let object = call
            .as_object()
            .ok_or_else(|| invalid("Chat Completions tool call must be an object"))?;
        let call_id = required_string(object.get("id"), "tool call id")?;
        if !seen_ids.insert(call_id) {
            return Err(invalid("Chat Completions tool call ids must be unique"));
        }
        let restored = restore_tool_call(object, projection)?;
        output.push(restored_call_item(
            response_id,
            index,
            call_id,
            &restored,
            "completed",
        ));
    }
    Ok(())
}

pub(super) fn restore_tool_call(
    call: &Map<String, Value>,
    projection: &ToolProjection,
) -> Result<RestoredToolCall, ProtocolError> {
    match call.get("type").and_then(Value::as_str) {
        Some("function") => {
            let function = call
                .get("function")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("function tool call payload is missing"))?;
            let name = required_string(function.get("name"), "tool call name")?;
            let arguments =
                required_string_allow_empty(function.get("arguments"), "tool call arguments")?;
            projection.restore_function_call(name, arguments)
        }
        Some("custom") => {
            let custom = call
                .get("custom")
                .and_then(Value::as_object)
                .ok_or_else(|| invalid("custom tool call payload is missing"))?;
            let name = required_string(custom.get("name"), "custom tool call name")?;
            let input = required_string_allow_empty(custom.get("input"), "custom tool call input")?;
            projection.restore_custom_call(name, input)
        }
        Some(kind) => Err(ProtocolError::unsupported_value("tool call type", kind)),
        None => Err(invalid("Chat Completions tool call type is required")),
    }
}

fn message_text(message: &Map<String, Value>) -> Result<Option<String>, ProtocolError> {
    let Some(content) = message.get("content") else {
        return Ok(None);
    };
    match content {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(value.clone())),
        Value::Array(parts) => parts
            .iter()
            .map(|part| match part {
                Value::String(value) => Ok(value.as_str()),
                Value::Object(object)
                    if object.get("type").and_then(Value::as_str) == Some("text") =>
                {
                    required_string(object.get("text"), "assistant text content")
                }
                _ => Err(invalid("assistant content part type is unsupported")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|parts| Some(parts.concat())),
        _ => Err(invalid(
            "assistant content must be null, a string, or an array",
        )),
    }
}

fn reasoning_text(
    message: &Map<String, Value>,
    profile: OpenAiChatCompletionsProfile,
) -> Result<Option<String>, ProtocolError> {
    let content = optional_reasoning(message.get("reasoning_content"))?;
    let reasoning = optional_reasoning(message.get("reasoning"))?;
    match profile.reasoning_response {
        OpenAiChatReasoningResponse::Unsupported if content.is_some() || reasoning.is_some() => {
            Err(invalid(
                "Chat target returned unsupported reasoning content",
            ))
        }
        OpenAiChatReasoningResponse::Unsupported => Ok(None),
        OpenAiChatReasoningResponse::ReasoningContent if reasoning.is_some() => Err(invalid(
            "Chat target returned an undeclared reasoning field",
        )),
        OpenAiChatReasoningResponse::ReasoningContent => Ok(content),
        OpenAiChatReasoningResponse::Reasoning if content.is_some() => Err(invalid(
            "Chat target returned an undeclared reasoning field",
        )),
        OpenAiChatReasoningResponse::Reasoning => Ok(reasoning),
        OpenAiChatReasoningResponse::ReasoningContentOrReasoning => match (content, reasoning) {
            (Some(left), Some(right)) if left != right => {
                Err(invalid("Chat target returned conflicting reasoning fields"))
            }
            (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
            (None, None) => Ok(None),
        },
    }
}

fn optional_reasoning(value: Option<&Value>) -> Result<Option<String>, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(invalid("Chat reasoning content must be a string")),
    }
}

fn required_string<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    required_string_allow_empty(value, field).and_then(|value| {
        (!value.is_empty())
            .then_some(value)
            .ok_or_else(|| invalid(field))
    })
}

fn required_string_allow_empty<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    value.and_then(Value::as_str).ok_or_else(|| invalid(field))
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
