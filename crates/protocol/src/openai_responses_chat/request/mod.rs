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
    conversation_start: usize,
}

impl ConvertedRequest {
    pub(super) fn conversation(&self) -> &[Value] {
        self.body["messages"]
            .as_array()
            .expect("converted request messages are an array")
            .get(self.conversation_start..)
            .expect("conversation start is within converted messages")
    }

    pub(super) fn into_conversation(mut self) -> Vec<Value> {
        let messages = self
            .body
            .as_object_mut()
            .expect("converted request body is an object")
            .remove("messages")
            .expect("converted request messages are an array");
        let Value::Array(mut messages) = messages else {
            unreachable!("converted request messages are an array");
        };
        messages.split_off(self.conversation_start)
    }
}

pub(super) fn convert(
    body: &Value,
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
    options::validate_client_metadata(source.get("client_metadata"))?;
    let mut conversation = previous;
    input::append_input(source.get("input"), &mut conversation)?;
    let mut messages = Vec::with_capacity(conversation.len().saturating_add(1));
    input::append_instructions(source.get("instructions"), &mut messages)?;
    let conversation_start = messages.len();
    messages.extend(conversation);

    let mut target = Map::new();
    target.insert("model".into(), Value::String(upstream_model.to_owned()));
    target.insert("messages".into(), Value::Array(messages));
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
        let converted = options::convert_text_config(text)?;
        if let Some(response_format) = converted.response_format {
            target.insert("response_format".into(), response_format);
        }
        if let Some(verbosity) = converted.verbosity {
            target.insert("verbosity".into(), verbosity);
        }
    }
    if let Some(value) = source.get("tools") {
        target.insert("tools".into(), tools::convert_tools(value)?);
    }
    if let Some(value) = source.get("tool_choice") {
        target.insert("tool_choice".into(), tools::convert_tool_choice(value)?);
    }

    Ok(ConvertedRequest {
        body: Value::Object(target),
        conversation_start,
    })
}

fn validate_top_level_fields(source: &Map<String, Value>) -> Result<(), ProtocolError> {
    if let Some(field) = source.keys().find(|field| {
        !(matches!(
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
                | "client_metadata"
                | "n"
        ) || PASSTHROUGH_FIELDS.contains(&field.as_str()))
    }) {
        return Err(ProtocolError::unsupported_field(None, field));
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
