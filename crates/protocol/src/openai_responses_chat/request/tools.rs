use serde_json::{Value, json};

use crate::ProtocolError;

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

pub(super) fn convert_tool_choice(value: &Value) -> Result<Value, ProtocolError> {
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
