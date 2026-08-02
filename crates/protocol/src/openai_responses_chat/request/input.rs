use serde_json::{Value, json};

use crate::ProtocolError;

use super::{history, invalid, required_string};

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
        Value::Array(items) => history::append_items(items, messages)?,
        _ => return Err(invalid("input must be a string or array")),
    }
    Ok(())
}

pub(super) fn convert_content(value: &Value, role: &str) -> Result<Value, ProtocolError> {
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
                let mut image = json!({"type":"image_url","image_url":{"url":url}});
                if let Some(detail) = image_detail(part.get("detail"))? {
                    image["image_url"]["detail"] = Value::String(detail.to_owned());
                }
                converted.push(image);
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

fn image_detail(value: Option<&Value>) -> Result<Option<&str>, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(detail))
            if matches!(detail.as_str(), "auto" | "low" | "high" | "original") =>
        {
            Ok(Some(detail))
        }
        Some(_) => Err(invalid(
            "input_image.detail must be auto, low, high, or original",
        )),
    }
}

pub(super) fn text_value(value: &Value) -> Result<String, ProtocolError> {
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

pub(super) fn reasoning_text(value: &Value) -> String {
    value
        .get("summary")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}
