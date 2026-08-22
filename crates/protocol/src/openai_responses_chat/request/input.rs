use serde_json::{Value, json};

use crate::{ProtocolError, api::OpenAiChatCompletionsProfile};

use super::{history, invalid, required_string};
use crate::openai_responses_chat::tool_projection::ToolProjection;

pub(super) fn append_instructions(
    instructions: Option<&Value>,
    messages: &mut Vec<Value>,
    profile: OpenAiChatCompletionsProfile,
) -> Result<(), ProtocolError> {
    let Some(instructions) = instructions else {
        return Ok(());
    };
    let text = text_value(instructions)?;
    if !text.is_empty() {
        messages.push(json!({"role":profile.instruction_role.as_str(),"content":text}));
    }
    Ok(())
}

pub(super) fn append_input(
    input: Option<&Value>,
    messages: &mut Vec<Value>,
    projection: &mut ToolProjection,
    profile: OpenAiChatCompletionsProfile,
) -> Result<(), ProtocolError> {
    let Some(input) = input else {
        return Err(invalid("input is required"));
    };
    match input {
        Value::String(text) => messages.push(json!({"role":"user","content":text})),
        Value::Array(items) => history::append_items(items, messages, projection, profile)?,
        _ => return Err(invalid("input must be a string or array")),
    }
    Ok(())
}

pub(super) fn convert_content(
    value: &Value,
    role: &str,
    profile: OpenAiChatCompletionsProfile,
) -> Result<Value, ProtocolError> {
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
                if !profile.supports_image_url {
                    return Err(invalid("Chat target does not support image_url content"));
                }
                let url = required_string(part.get("image_url"), "input_image.image_url")?;
                let mut image = json!({"type":"image_url","image_url":{"url":url}});
                if let Some(detail) = image_detail(part.get("detail"))? {
                    if !profile.supports_image_detail {
                        return Err(invalid("Chat target does not support image detail"));
                    }
                    image["image_url"]["detail"] = Value::String(detail.to_owned());
                }
                converted.push(image);
            }
            Some("input_audio") if role == "user" => {
                if !profile.supports_input_audio {
                    return Err(invalid("Chat target does not support input_audio content"));
                }
                let audio = part
                    .get("input_audio")
                    .and_then(Value::as_object)
                    .ok_or_else(|| invalid("input_audio.input_audio must be an object"))?;
                let data = required_string(audio.get("data"), "input_audio.data")?;
                let format = required_string(audio.get("format"), "input_audio.format")?;
                if !matches!(format, "wav" | "mp3") {
                    return Err(invalid("input_audio.format must be wav or mp3"));
                }
                converted.push(json!({
                    "type":"input_audio","input_audio":{"data":data,"format":format}
                }));
            }
            Some("input_file") if role == "user" => {
                if !profile.supports_file {
                    return Err(invalid("Chat target does not support file content"));
                }
                let mut file = serde_json::Map::new();
                for field in ["file_id", "file_data", "filename"] {
                    if let Some(value) = part.get(field) {
                        let value = required_string(Some(value), "input_file field")?;
                        file.insert(field.into(), Value::String(value.to_owned()));
                    }
                }
                let sources = usize::from(file.contains_key("file_id"))
                    + usize::from(file.contains_key("file_data"));
                if sources != 1 {
                    return Err(invalid(
                        "input_file must contain exactly one of file_id or file_data",
                    ));
                }
                converted.push(json!({"type":"file","file":file}));
            }
            Some(kind) => {
                return Err(ProtocolError::unsupported_value(
                    "message content part type",
                    kind,
                ));
            }
            None => return Err(invalid("message content part type is required")),
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
