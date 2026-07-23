use serde_json::{Map, Value, json};

use crate::ProtocolError;

const PASSTHROUGH_FIELDS: &[&str] = &[
    "frequency_penalty",
    "logit_bias",
    "logprobs",
    "metadata",
    "parallel_tool_calls",
    "presence_penalty",
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
    let mut messages = previous;
    append_instructions(source.get("instructions"), &mut messages)?;
    append_input(source.get("input"), &mut messages)?;

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
    if let Some(reasoning) = source.get("reasoning") {
        target.insert("reasoning_effort".into(), convert_reasoning(reasoning)?);
    }
    if let Some(text) = source.get("text") {
        target.insert("response_format".into(), convert_text_config(text)?);
    }
    if let Some(tools) = source.get("tools") {
        target.insert("tools".into(), convert_tools(tools)?);
    }
    if let Some(choice) = source.get("tool_choice") {
        target.insert("tool_choice".into(), convert_tool_choice(choice)?);
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

fn convert_reasoning(value: &Value) -> Result<Value, ProtocolError> {
    let reasoning = value
        .as_object()
        .ok_or_else(|| invalid("reasoning must be an object"))?;
    if reasoning.keys().any(|field| field != "effort") {
        return Err(invalid("only reasoning.effort is supported by this bridge"));
    }
    reasoning
        .get("effort")
        .filter(|effort| effort.is_string())
        .cloned()
        .ok_or_else(|| invalid("reasoning.effort must be a string"))
}

fn convert_text_config(value: &Value) -> Result<Value, ProtocolError> {
    let text = value
        .as_object()
        .ok_or_else(|| invalid("text must be an object"))?;
    if text.keys().any(|field| field != "format") {
        return Err(invalid("only text.format is supported by this bridge"));
    }
    let format = text
        .get("format")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("text.format must be an object"))?;
    let kind = required_string(format.get("type"), "text.format.type")?;
    match kind {
        "text" | "json_object" => Ok(json!({"type":kind})),
        "json_schema" => convert_json_schema(format),
        _ => Err(invalid("text.format type is not supported by this bridge")),
    }
}

fn convert_json_schema(format: &Map<String, Value>) -> Result<Value, ProtocolError> {
    if format.keys().any(|field| {
        !matches!(
            field.as_str(),
            "type" | "name" | "description" | "schema" | "strict"
        )
    }) {
        return Err(invalid(
            "text.format json_schema contains unsupported fields",
        ));
    }
    let name = required_string(format.get("name"), "text.format.name")?;
    let schema = format
        .get("schema")
        .filter(|schema| schema.is_object())
        .ok_or_else(|| invalid("text.format.schema must be an object"))?;
    let mut converted = Map::new();
    converted.insert("name".into(), Value::String(name.to_owned()));
    converted.insert("schema".into(), schema.clone());
    for field in ["description", "strict"] {
        if let Some(value) = format.get(field) {
            converted.insert(field.into(), value.clone());
        }
    }
    Ok(json!({"type":"json_schema","json_schema":converted}))
}

fn append_instructions(
    instructions: Option<&Value>,
    messages: &mut Vec<Value>,
) -> Result<(), ProtocolError> {
    let Some(instructions) = instructions else {
        return Ok(());
    };
    let text = text_value(instructions)?;
    if !text.is_empty() {
        messages.push(json!({"role":"system","content":text}));
    }
    Ok(())
}

fn append_input(input: Option<&Value>, messages: &mut Vec<Value>) -> Result<(), ProtocolError> {
    let Some(input) = input else {
        return Err(invalid("input is required"));
    };
    match input {
        Value::String(text) => messages.push(json!({"role":"user","content":text})),
        Value::Array(items) => {
            for item in items {
                append_item(item, messages)?;
            }
        }
        _ => return Err(invalid("input must be a string or array")),
    }
    Ok(())
}

fn append_item(item: &Value, messages: &mut Vec<Value>) -> Result<(), ProtocolError> {
    let object = item
        .as_object()
        .ok_or_else(|| invalid("input items must be objects"))?;
    match object.get("type").and_then(Value::as_str) {
        Some("message") | None if object.contains_key("role") => {
            let role = object
                .get("role")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("message role is required"))?;
            let role = match role {
                "developer" | "system" => "system",
                "user" => "user",
                "assistant" => "assistant",
                _ => return Err(invalid("message role is not supported by Chat Completions")),
            };
            let content = convert_content(
                object
                    .get("content")
                    .ok_or_else(|| invalid("message content is required"))?,
                role,
            )?;
            messages.push(json!({"role":role,"content":content}));
        }
        Some("input_text") => {
            let text = required_string(object.get("text"), "input_text.text")?;
            messages.push(json!({"role":"user","content":text}));
        }
        Some("function_call") => {
            let call_id = required_string(object.get("call_id"), "function_call.call_id")?;
            let name = required_string(object.get("name"), "function_call.name")?;
            let arguments = required_string(object.get("arguments"), "function_call.arguments")?;
            messages.push(json!({
                "role":"assistant",
                "content": Value::Null,
                "tool_calls":[{
                    "id":call_id,
                    "type":"function",
                    "function":{"name":name,"arguments":arguments}
                }]
            }));
        }
        Some("function_call_output") => {
            let call_id = required_string(object.get("call_id"), "function_call_output.call_id")?;
            let output = object
                .get("output")
                .ok_or_else(|| invalid("function_call_output.output is required"))?;
            messages.push(json!({
                "role":"tool",
                "tool_call_id":call_id,
                "content":text_value(output)?
            }));
        }
        Some("reasoning") => {
            let text = reasoning_text(item);
            if !text.is_empty() {
                messages.push(
                    json!({"role":"assistant","reasoning_content":text,"content":Value::Null}),
                );
            }
        }
        Some(_) => {
            return Err(invalid(
                "Responses input item is not supported by this bridge",
            ));
        }
        None => return Err(invalid("Responses input item type is required")),
    }
    Ok(())
}

fn convert_content(value: &Value, role: &str) -> Result<Value, ProtocolError> {
    if let Some(text) = value.as_str() {
        return Ok(Value::String(text.to_owned()));
    }
    let parts = value
        .as_array()
        .ok_or_else(|| invalid("message content must be a string or array"))?;
    let mut converted = Vec::new();
    for part in parts {
        let kind = part.get("type").and_then(Value::as_str);
        match kind {
            Some("input_text" | "output_text" | "text") => {
                converted.push(json!({
                    "type":"text",
                    "text":required_string(part.get("text"), "content text")?
                }));
            }
            Some("input_image") if role == "user" => {
                let url = required_string(part.get("image_url"), "input_image.image_url")?;
                converted.push(json!({"type":"image_url","image_url":{"url":url}}));
            }
            _ => {
                return Err(invalid(
                    "message content part is not supported by this bridge",
                ));
            }
        }
    }
    if role == "assistant" {
        return Ok(Value::String(
            converted
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(""),
        ));
    }
    Ok(Value::Array(converted))
}

fn convert_tools(value: &Value) -> Result<Value, ProtocolError> {
    let tools = value
        .as_array()
        .ok_or_else(|| invalid("tools must be an array"))?;
    tools
        .iter()
        .map(|tool| {
            let object = tool
                .as_object()
                .ok_or_else(|| invalid("tool must be an object"))?;
            if object.keys().any(|field| {
                !matches!(
                    field.as_str(),
                    "type" | "name" | "description" | "parameters" | "strict"
                )
            }) {
                return Err(invalid("function tool contains unsupported fields"));
            }
            if object.get("type").and_then(Value::as_str) != Some("function") {
                return Err(invalid("only function tools are supported by this bridge"));
            }
            let name = required_string(object.get("name"), "tool.name")?;
            Ok(json!({
                "type":"function",
                "function":{
                    "name":name,
                    "description":object.get("description").cloned().unwrap_or(Value::Null),
                    "parameters":object.get("parameters").cloned().unwrap_or_else(|| json!({"type":"object","properties":{}})),
                    "strict":object.get("strict").cloned().unwrap_or(Value::Bool(false))
                }
            }))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}

fn convert_tool_choice(value: &Value) -> Result<Value, ProtocolError> {
    if let Some(choice) = value.as_str() {
        return match choice {
            "none" | "auto" | "required" => Ok(value.clone()),
            _ => Err(invalid(
                "tool_choice string is not supported by this bridge",
            )),
        };
    }
    let choice = value
        .as_object()
        .ok_or_else(|| invalid("tool_choice must be a string or object"))?;
    if choice
        .keys()
        .any(|field| !matches!(field.as_str(), "type" | "name"))
        || choice.get("type").and_then(Value::as_str) != Some("function")
    {
        return Err(invalid(
            "tool_choice object is not supported by this bridge",
        ));
    }
    let name = required_string(choice.get("name"), "tool_choice.name")?;
    Ok(json!({"type":"function","function":{"name":name}}))
}

fn text_value(value: &Value) -> Result<String, ProtocolError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .or_else(|| value.get("text").and_then(Value::as_str))
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("text array contains an unsupported value"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join("\n")),
        other => serde_json::to_string(other)
            .map_err(|_| invalid("value could not be represented as text")),
    }
}

fn reasoning_text(value: &Value) -> String {
    value
        .get("summary")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
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
