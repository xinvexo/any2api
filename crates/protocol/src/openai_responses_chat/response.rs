use std::fmt::Display;

use any2api_domain::TokenUsage;
use serde_json::{Map, Value, json};

use crate::ProtocolError;

pub(super) struct ConvertedResponse {
    pub(super) body: Value,
    pub(super) assistant_message: Value,
}

pub(super) fn convert(body: Value, response_id: &str) -> Result<ConvertedResponse, ProtocolError> {
    let choices = body
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Chat Completions response has no choices"))?;
    if choices.len() != 1 {
        return Err(invalid(
            "Chat Completions response must contain exactly one choice",
        ));
    }
    let choice = &choices[0];
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Chat Completions choice has no message"))?;
    validate_message(message)?;
    let finish_reason = choice.get("finish_reason").and_then(Value::as_str);
    let incomplete_reason = incomplete_reason(finish_reason);
    let status = if incomplete_reason.is_some() {
        "incomplete"
    } else {
        "completed"
    };
    let mut output = Vec::new();
    if let Some(reasoning) = reasoning_text(message)
        && !reasoning.is_empty()
    {
        output.push(reasoning_item(response_id, &reasoning));
    }
    if let Some(text) = message_text(message)
        && !text.is_empty()
    {
        output.push(message_item(response_id, &text));
    }
    append_tool_calls(message, response_id, &mut output)?;

    let mut response = json!({
        "id":response_id,
        "object":"response",
        "created_at":body.get("created").and_then(Value::as_u64).unwrap_or(0),
        "status":status,
        "model":body.get("model").cloned().unwrap_or(Value::Null),
        "output":output,
        "parallel_tool_calls":true,
        "usage":responses_usage(body.get("usage")),
        "error":Value::Null,
        "incomplete_details":Value::Null
    });
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

pub(super) fn responses_usage(value: Option<&Value>) -> Value {
    let prompt = token(value, &["prompt_tokens"]).unwrap_or(0);
    let completion = token(value, &["completion_tokens"]).unwrap_or(0);
    let cached = token(value, &["prompt_tokens_details", "cached_tokens"]).unwrap_or(0);
    let reasoning = token(value, &["completion_tokens_details", "reasoning_tokens"]).unwrap_or(0);
    json!({
        "input_tokens":prompt,
        "output_tokens":completion,
        "total_tokens":prompt.saturating_add(completion),
        "input_tokens_details":{"cached_tokens":cached},
        "output_tokens_details":{"reasoning_tokens":reasoning}
    })
}

pub(super) fn token_usage(value: Option<&Value>) -> TokenUsage {
    TokenUsage::new(
        token(value, &["prompt_tokens"]),
        token(value, &["completion_tokens"]),
        token(value, &["prompt_tokens_details", "cached_tokens"]),
    )
}

pub(super) fn message_item(response_id: &str, text: &str) -> Value {
    json!({
        "id":message_item_id(response_id),
        "type":"message",
        "status":"completed",
        "role":"assistant",
        "content":[{
            "type":"output_text",
            "text":text,
            "annotations":[]
        }]
    })
}

pub(super) fn reasoning_item(response_id: &str, text: &str) -> Value {
    json!({
        "id":reasoning_item_id(response_id),
        "type":"reasoning",
        "status":"completed",
        "summary":[{"type":"summary_text","text":text}]
    })
}

pub(super) fn function_call_item(
    response_id: &str,
    index: impl Display,
    call_id: &str,
    name: &str,
    arguments: &str,
) -> Value {
    json!({
        "id":function_call_item_id(response_id, index),
        "type":"function_call",
        "status":"completed",
        "call_id":call_id,
        "name":name,
        "arguments":arguments
    })
}

pub(super) fn message_item_id(response_id: &str) -> String {
    format!("msg_{response_id}")
}

pub(super) fn reasoning_item_id(response_id: &str) -> String {
    format!("rs_{response_id}")
}

pub(super) fn function_call_item_id(response_id: &str, index: impl Display) -> String {
    format!("fc_{response_id}_{index}")
}

fn append_tool_calls(
    message: &Map<String, Value>,
    response_id: &str,
    output: &mut Vec<Value>,
) -> Result<(), ProtocolError> {
    let Some(calls) = message.get("tool_calls").and_then(Value::as_array) else {
        return Ok(());
    };
    for (index, call) in calls.iter().enumerate() {
        let call_id = required_string(call.get("id"), "tool call id")?;
        let function = call
            .get("function")
            .ok_or_else(|| invalid("tool call function is missing"))?;
        let name = required_string(function.get("name"), "tool call name")?;
        let arguments = required_string(function.get("arguments"), "tool call arguments")?;
        output.push(function_call_item(
            response_id,
            index,
            call_id,
            name,
            arguments,
        ));
    }
    Ok(())
}

fn message_text(message: &Map<String, Value>) -> Option<String> {
    match message.get("content")? {
        Value::String(value) => Some(value.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|part| {
                    part.as_str()
                        .or_else(|| part.get("text").and_then(Value::as_str))
                })
                .collect::<Vec<_>>()
                .join(""),
        ),
        _ => None,
    }
}

fn reasoning_text(message: &Map<String, Value>) -> Option<String> {
    message
        .get("reasoning_content")
        .or_else(|| message.get("reasoning"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn token(mut value: Option<&Value>, path: &[&str]) -> Option<u64> {
    for part in path {
        value = value?.get(*part);
    }
    value?.as_u64()
}

fn required_string<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(field))
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
