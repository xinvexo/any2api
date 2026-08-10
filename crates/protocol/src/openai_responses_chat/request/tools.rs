use serde_json::{Value, json};

use crate::ProtocolError;

use super::super::capabilities::CAPABILITIES;
use super::{invalid, required_string};

pub(super) fn convert_tools(value: &Value) -> Result<Value, ProtocolError> {
    let tools = value
        .as_array()
        .ok_or_else(|| invalid("tools must be an array"))?;
    tools
        .iter()
        .map(|tool| {
            let object = tool
                .as_object()
                .ok_or_else(|| invalid("tool must be an object"))?;
            let kind = required_string(object.get("type"), "tool.type")?;
            if !CAPABILITIES.supports_tool_type(kind) {
                return Err(ProtocolError::unsupported_value("tool type", kind));
            }
            if let Some(field) = object.keys().find(|field| {
                !matches!(
                    field.as_str(),
                    "type" | "name" | "description" | "parameters" | "strict"
                )
            }) {
                return Err(ProtocolError::unsupported_field(Some("tools[]"), field));
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

pub(super) fn convert_tool_choice(value: &Value) -> Result<Value, ProtocolError> {
    if let Some(choice) = value.as_str() {
        return match choice {
            "none" | "auto" | "required" => Ok(value.clone()),
            _ => Err(ProtocolError::unsupported_value("tool_choice", choice)),
        };
    }
    let choice = value
        .as_object()
        .ok_or_else(|| invalid("tool_choice must be a string or object"))?;
    if let Some(field) = choice
        .keys()
        .find(|field| !matches!(field.as_str(), "type" | "name"))
    {
        return Err(ProtocolError::unsupported_field(Some("tool_choice"), field));
    }
    let kind = required_string(choice.get("type"), "tool_choice.type")?;
    if !CAPABILITIES.supports_tool_type(kind) {
        return Err(ProtocolError::unsupported_value("tool_choice type", kind));
    }
    let name = required_string(choice.get("name"), "tool_choice.name")?;
    Ok(json!({"type":"function","function":{"name":name}}))
}
