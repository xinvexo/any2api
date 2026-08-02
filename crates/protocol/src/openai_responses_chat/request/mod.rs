mod history;
mod input;
mod options;
mod tools;

use serde_json::{Map, Value, json};

use crate::ProtocolError;

const PASSTHROUGH_FIELDS: &[&str] = &[
    "frequency_penalty",
    "logit_bias",
    "logprobs",
    "metadata",
    "parallel_tool_calls",
    "presence_penalty",
    "prompt_cache_key",
    "seed",
    "service_tier",
    "stop",
    "store",
    "temperature",
    "top_logprobs",
    "top_p",
    "user",
];

pub(super) struct ConvertedRequest {
    pub(super) body: Value,
    pub(super) conversation: Vec<Value>,
}

pub(super) fn convert(
    body: Value,
    upstream_model: &str,
    previous: Vec<Value>,
) -> Result<ConvertedRequest, ProtocolError> {
    let source = body
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    validate_top_level_fields(source)?;
    reject_multiple_choices(source)?;
    validate_prompt_cache_key(source.get("prompt_cache_key"))?;
    options::validate_include(source.get("include"))?;
    let mut messages = previous;
    input::append_instructions(source.get("instructions"), &mut messages)?;
    input::append_input(source.get("input"), &mut messages)?;

    let mut target = Map::new();
    target.insert("model".into(), Value::String(upstream_model.to_owned()));
    target.insert("messages".into(), Value::Array(messages.clone()));
    for field in PASSTHROUGH_FIELDS {
        if let Some(value) = source.get(*field) {
            target.insert((*field).into(), value.clone());
        }
    }
    if let Some(value) = source.get("max_output_tokens") {
        target.insert("max_tokens".into(), value.clone());
    }
    if let Some(value) = source.get("stream") {
        target.insert("stream".into(), value.clone());
        if value == &Value::Bool(true) {
            target.insert("stream_options".into(), json!({"include_usage": true}));
        }
    }
    if let Some(reasoning) = source.get("reasoning")
        && let Some(effort) = options::convert_reasoning(reasoning)?
    {
        target.insert("reasoning_effort".into(), effort);
    }
    if let Some(text) = source.get("text") {
        target.insert(
            "response_format".into(),
            options::convert_text_config(text)?,
        );
    }
    if let Some(value) = source.get("tools") {
        target.insert("tools".into(), tools::convert_tools(value)?);
    }
    if let Some(value) = source.get("tool_choice") {
        target.insert("tool_choice".into(), tools::convert_tool_choice(value)?);
    }

    Ok(ConvertedRequest {
        body: Value::Object(target),
        conversation: messages,
    })
}

fn validate_top_level_fields(source: &Map<String, Value>) -> Result<(), ProtocolError> {
    for field in source.keys() {
        let supported = matches!(
            field.as_str(),
            "model"
                | "input"
                | "instructions"
                | "previous_response_id"
                | "include"
                | "stream"
                | "max_output_tokens"
                | "reasoning"
                | "text"
                | "tools"
                | "tool_choice"
                | "n"
        ) || PASSTHROUGH_FIELDS.contains(&field.as_str());
        if !supported {
            return Err(ProtocolError::InvalidPayload(format!(
                "Responses field {field:?} is not supported by the Chat Completions bridge"
            )));
        }
    }
    Ok(())
}

fn reject_multiple_choices(source: &Map<String, Value>) -> Result<(), ProtocolError> {
    if let Some(value) = source.get("n")
        && value.as_u64() != Some(1)
    {
        return Err(invalid("Responses bridge supports exactly one choice"));
    }
    Ok(())
}

fn validate_prompt_cache_key(value: Option<&Value>) -> Result<(), ProtocolError> {
    match value {
        None | Some(Value::String(_)) => Ok(()),
        Some(_) => Err(invalid("prompt_cache_key must be a string")),
    }
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
