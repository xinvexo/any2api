use serde_json::{Map, Value, json};

use crate::ProtocolError;

use super::{invalid, required_string};

pub(super) fn validate_include(value: Option<&Value>) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        return Ok(());
    };
    let include = value
        .as_array()
        .ok_or_else(|| invalid("include must be an array"))?;
    if include
        .iter()
        .any(|item| item.as_str() != Some("reasoning.encrypted_content"))
    {
        return Err(invalid(
            "only reasoning.encrypted_content can be included by this bridge",
        ));
    }
    Ok(())
}

pub(super) fn convert_reasoning(value: &Value) -> Result<Option<Value>, ProtocolError> {
    let reasoning = value
        .as_object()
        .ok_or_else(|| invalid("reasoning must be an object"))?;
    if reasoning
        .keys()
        .any(|field| !matches!(field.as_str(), "effort" | "summary"))
    {
        return Err(invalid(
            "only reasoning.effort and reasoning.summary are supported by this bridge",
        ));
    }
    if let Some(summary) = reasoning.get("summary") {
        match summary.as_str() {
            Some("auto" | "concise" | "detailed") => {}
            _ => {
                return Err(invalid(
                    "reasoning.summary must be auto, concise, or detailed",
                ));
            }
        }
    }
    match reasoning.get("effort") {
        Some(effort) if effort.is_string() => Ok(Some(effort.clone())),
        Some(_) => Err(invalid("reasoning.effort must be a string")),
        None if reasoning.contains_key("summary") => Ok(None),
        None => Err(invalid(
            "reasoning must contain effort or summary for this bridge",
        )),
    }
}

pub(super) fn convert_text_config(value: &Value) -> Result<Value, ProtocolError> {
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
