use serde_json::{Map, Value, json};

use crate::ProtocolError;

use super::ParsedTool;
use crate::openai_responses_chat::tool_projection::ToolIdentity;

pub(super) fn top_level(value: &Value, target: &mut Vec<ParsedTool>) -> Result<(), ProtocolError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("tool must be an object"))?;
    let kind = required_string(object.get("type"), "tool.type")?;
    match kind {
        "function" => target.push(parse_function(object, None)?),
        "custom" => target.push(parse_custom(object, None)?),
        "namespace" => parse_namespace(object, target)?,
        "tool_search" => target.push(parse_tool_search(object)?),
        other => return Err(ProtocolError::unsupported_value("tool type", other)),
    }
    Ok(())
}

fn parse_namespace(
    object: &Map<String, Value>,
    target: &mut Vec<ParsedTool>,
) -> Result<(), ProtocolError> {
    reject_fields(object, "tools[]", &["type", "name", "description", "tools"])?;
    let namespace = required_string(object.get("name"), "namespace.name")?.to_owned();
    let description = optional_string(object.get("description"), "namespace.description")?;
    let tools = object
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("namespace.tools must be an array"))?;
    if tools.is_empty() {
        return Err(invalid("namespace.tools must not be empty"));
    }
    for tool in tools {
        let child = tool
            .as_object()
            .ok_or_else(|| invalid("namespace tool must be an object"))?;
        let kind = required_string(child.get("type"), "namespace tool.type")?;
        let mut parsed = match kind {
            "function" => parse_function(child, Some(&namespace))?,
            "custom" => parse_custom(child, Some(&namespace))?,
            other => {
                return Err(ProtocolError::unsupported_value(
                    "namespace tool type",
                    other,
                ));
            }
        };
        prepend_description(&mut parsed, description.as_deref());
        target.push(parsed);
    }
    Ok(())
}

fn parse_function(
    object: &Map<String, Value>,
    namespace: Option<&str>,
) -> Result<ParsedTool, ProtocolError> {
    reject_fields(
        object,
        "tools[]",
        &[
            "type",
            "name",
            "description",
            "parameters",
            "strict",
            "defer_loading",
            "output_schema",
            "allowed_callers",
        ],
    )?;
    let deferred = validate_loading_fields(object)?;
    if object
        .get("output_schema")
        .is_some_and(|value| !value.is_null())
    {
        return Err(ProtocolError::InvalidPayload(
            "function output_schema has no Chat Completions equivalent".into(),
        ));
    }
    let name = required_string(object.get("name"), "function tool.name")?.to_owned();
    let identity = namespace.map_or_else(
        || ToolIdentity::Function { name: name.clone() },
        |namespace| ToolIdentity::NamespaceFunction {
            namespace: namespace.to_owned(),
            name: name.clone(),
        },
    );
    let description = optional_string(object.get("description"), "function tool.description")?;
    let parameters = match object.get("parameters") {
        None => json!({"type":"object","properties":{}}),
        Some(value) if value.is_object() => value.clone(),
        Some(_) => return Err(invalid("function tool.parameters must be an object")),
    };
    let strict = optional_bool(object.get("strict"), "function tool.strict")?.unwrap_or(false);
    Ok(ParsedTool::Function {
        identity,
        description,
        parameters,
        strict,
        deferred,
    })
}

fn parse_custom(
    object: &Map<String, Value>,
    namespace: Option<&str>,
) -> Result<ParsedTool, ProtocolError> {
    reject_fields(
        object,
        "tools[]",
        &["type", "name", "description", "format", "defer_loading"],
    )?;
    let deferred = validate_loading_fields(object)?;
    let name = required_string(object.get("name"), "custom tool.name")?.to_owned();
    let identity = namespace.map_or_else(
        || ToolIdentity::Custom { name: name.clone() },
        |namespace| ToolIdentity::NamespaceCustom {
            namespace: namespace.to_owned(),
            name: name.clone(),
        },
    );
    let description = optional_string(object.get("description"), "custom tool.description")?;
    let format = match object.get("format") {
        None => None,
        Some(value) if value.is_object() => Some(value.clone()),
        Some(_) => return Err(invalid("custom tool.format must be an object")),
    };
    Ok(ParsedTool::Custom {
        identity,
        description,
        format,
        deferred,
    })
}

fn parse_tool_search(object: &Map<String, Value>) -> Result<ParsedTool, ProtocolError> {
    reject_fields(
        object,
        "tools[]",
        &["type", "execution", "description", "parameters"],
    )?;
    if required_string(object.get("execution"), "tool_search.execution")? != "client" {
        return Err(invalid("only client-executed tool_search is supported"));
    }
    let description = required_string(object.get("description"), "tool_search.description")?;
    let parameters = object
        .get("parameters")
        .filter(|value| value.is_object())
        .ok_or_else(|| invalid("tool_search.parameters must be an object"))?;
    Ok(ParsedTool::ToolSearch {
        description: description.to_owned(),
        parameters: parameters.clone(),
    })
}

fn validate_loading_fields(object: &Map<String, Value>) -> Result<bool, ProtocolError> {
    let deferred =
        optional_bool(object.get("defer_loading"), "tool.defer_loading")?.unwrap_or(false);
    let Some(callers) = object.get("allowed_callers") else {
        return Ok(deferred);
    };
    let callers = callers
        .as_array()
        .ok_or_else(|| invalid("tool.allowed_callers must be an array"))?;
    if callers.iter().any(|caller| !caller.is_string()) {
        return Err(invalid("tool.allowed_callers values must be strings"));
    }
    if !deferred
        && !callers
            .iter()
            .any(|caller| caller.as_str() == Some("direct"))
    {
        return Err(ProtocolError::InvalidPayload(
            "tool.allowed_callers excludes direct model calls".into(),
        ));
    }
    Ok(deferred)
}

fn prepend_description(tool: &mut ParsedTool, namespace: Option<&str>) {
    let Some(namespace) = namespace.filter(|value| !value.is_empty()) else {
        return;
    };
    let description = match tool {
        ParsedTool::Function { description, .. } | ParsedTool::Custom { description, .. } => {
            description
        }
        ParsedTool::ToolSearch { .. } => return,
    };
    *description = Some(description.as_ref().map_or_else(
        || namespace.to_owned(),
        |child| format!("{namespace}\n\n{child}"),
    ));
}

fn reject_fields(
    object: &Map<String, Value>,
    scope: &str,
    allowed: &[&str],
) -> Result<(), ProtocolError> {
    if let Some(field) = object
        .keys()
        .find(|field| !allowed.contains(&field.as_str()))
    {
        return Err(ProtocolError::unsupported_field(Some(scope), field));
    }
    Ok(())
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

fn optional_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, ProtocolError> {
    match value {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(invalid(field)),
    }
}

fn optional_bool(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<bool>, ProtocolError> {
    match value {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(invalid(field)),
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
