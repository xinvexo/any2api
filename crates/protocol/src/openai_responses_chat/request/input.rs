use serde_json::{Value, json};

use crate::ProtocolError;

use super::{invalid, required_string};

pub(super) fn append_instructions(
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

pub(super) fn append_input(
    input: Option<&Value>,
    messages: &mut Vec<Value>,
) -> Result<(), ProtocolError> {
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
