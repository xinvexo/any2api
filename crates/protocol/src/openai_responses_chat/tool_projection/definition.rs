mod parse;

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::{ProtocolError, api::OpenAiChatCustomToolMode};

use super::{ChatToolKind, ToolIdentity, ToolProjection};

pub(super) fn configure(
    projection: &mut ToolProjection,
    value: Option<&Value>,
) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        return Ok(());
    };
    let tools = value
        .as_array()
        .ok_or_else(|| invalid("tools must be an array"))?;
    let mut parsed = Vec::new();
    for tool in tools {
        parse::top_level(tool, &mut parsed)?;
    }
    let has_tool_search = parsed
        .iter()
        .any(|tool| matches!(tool, ParsedTool::ToolSearch { .. }));
    if parsed.iter().any(ParsedTool::is_deferred) && !has_tool_search {
        return Err(ProtocolError::InvalidPayload(
            "deferred tools require a client-executed tool_search tool".into(),
        ));
    }

    let mut seen = BTreeSet::new();
    if let Some(identity) = parsed
        .iter()
        .map(ParsedTool::identity)
        .find(|identity| !seen.insert((*identity).clone()))
    {
        return Err(ProtocolError::InvalidPayload(format!(
            "tool `{}` is declared more than once",
            identity.diagnostic_name()
        )));
    }

    let mut identities = parsed
        .iter()
        .map(ParsedTool::identity)
        .cloned()
        .collect::<Vec<_>>();
    identities.sort();
    for identity in identities {
        projection.ensure_identity(identity)?;
    }
    for tool in parsed {
        if tool.is_deferred() {
            projection.register_deferred(tool.identity().clone());
            continue;
        }
        activate(projection, tool)?;
    }
    Ok(())
}

pub(super) fn activate_loaded(
    projection: &mut ToolProjection,
    tools: &[Value],
) -> Result<(), ProtocolError> {
    let mut parsed = Vec::new();
    for tool in tools {
        parse::top_level(tool, &mut parsed)?;
    }
    for tool in parsed {
        if matches!(tool, ParsedTool::ToolSearch { .. }) {
            return Err(ProtocolError::InvalidPayload(
                "tool_search_output cannot dynamically load another tool_search".into(),
            ));
        }
        if !tool.is_deferred() {
            return Err(ProtocolError::InvalidPayload(
                "tool_search_output tools must retain defer_loading=true".into(),
            ));
        }
        let identity = tool.identity().clone();
        if !projection.is_registered_deferred(&identity) {
            return Err(ProtocolError::InvalidPayload(format!(
                "tool_search_output returned unregistered tool `{}`",
                identity.diagnostic_name()
            )));
        }
        activate(projection, tool)?;
    }
    Ok(())
}

fn activate(projection: &mut ToolProjection, tool: ParsedTool) -> Result<(), ProtocolError> {
    let identity = tool.identity().clone();
    let chat_name = projection
        .original_to_chat
        .get(&identity)
        .cloned()
        .ok_or_else(|| ProtocolError::Internal("tool projection was not allocated".into()))?;
    let definition = tool.into_chat_definition(projection, &chat_name)?;
    projection.activate(&identity, definition)?;
    Ok(())
}

pub(super) fn project_tool_choice(
    projection: &ToolProjection,
    value: &Value,
) -> Result<Value, ProtocolError> {
    if let Some(choice) = value.as_str() {
        return match choice {
            "none" | "auto" | "required" => Ok(value.clone()),
            _ => Err(ProtocolError::unsupported_value("tool_choice", choice)),
        };
    }
    let choice = value
        .as_object()
        .ok_or_else(|| invalid("tool_choice must be a string or object"))?;
    reject_fields(choice, "tool_choice", &["type", "name", "namespace"])?;
    let kind = required_string(choice.get("type"), "tool_choice.type")?;
    let identity = match kind {
        "function" => function_choice(choice, false)?,
        "custom" => function_choice(choice, true)?,
        "tool_search" => {
            if choice.contains_key("name") || choice.contains_key("namespace") {
                return Err(invalid(
                    "tool_search tool_choice cannot contain name or namespace",
                ));
            }
            ToolIdentity::ToolSearch
        }
        "namespace" => {
            return Err(ProtocolError::InvalidPayload(
                "namespace-wide tool_choice has no Chat Completions equivalent".into(),
            ));
        }
        other => return Err(ProtocolError::unsupported_value("tool_choice type", other)),
    };
    let chat_name = projection.active_name(&identity)?;
    match identity.chat_kind(projection.profile) {
        ChatToolKind::Function => Ok(json!({"type":"function","function":{"name":chat_name}})),
        ChatToolKind::Custom => Ok(json!({"type":"custom","custom":{"name":chat_name}})),
    }
}

fn function_choice(
    choice: &Map<String, Value>,
    custom: bool,
) -> Result<ToolIdentity, ProtocolError> {
    let name = required_string(choice.get("name"), "tool_choice.name")?.to_owned();
    Ok(match (custom, choice.get("namespace")) {
        (false, None) => ToolIdentity::Function { name },
        (true, None) => ToolIdentity::Custom { name },
        (false, Some(namespace)) => ToolIdentity::NamespaceFunction {
            namespace: required_string(Some(namespace), "tool_choice.namespace")?.to_owned(),
            name,
        },
        (true, Some(namespace)) => ToolIdentity::NamespaceCustom {
            namespace: required_string(Some(namespace), "tool_choice.namespace")?.to_owned(),
            name,
        },
    })
}

pub(super) enum ParsedTool {
    Function {
        identity: ToolIdentity,
        description: Option<String>,
        parameters: Value,
        strict: bool,
        deferred: bool,
    },
    Custom {
        identity: ToolIdentity,
        description: Option<String>,
        format: Option<Value>,
        deferred: bool,
    },
    ToolSearch {
        description: String,
        parameters: Value,
    },
}

impl ParsedTool {
    fn is_deferred(&self) -> bool {
        match self {
            Self::Function { deferred, .. } | Self::Custom { deferred, .. } => *deferred,
            Self::ToolSearch { .. } => false,
        }
    }

    fn identity(&self) -> &ToolIdentity {
        match self {
            Self::Function { identity, .. } | Self::Custom { identity, .. } => identity,
            Self::ToolSearch { .. } => &ToolIdentity::ToolSearch,
        }
    }

    fn into_chat_definition(
        self,
        projection: &ToolProjection,
        chat_name: &str,
    ) -> Result<Value, ProtocolError> {
        match self {
            Self::Function {
                description,
                parameters,
                strict,
                ..
            } => {
                let mut function = Map::new();
                function.insert("name".into(), Value::String(chat_name.to_owned()));
                if let Some(description) = description {
                    function.insert("description".into(), Value::String(description));
                }
                function.insert("parameters".into(), parameters);
                function.insert("strict".into(), Value::Bool(strict));
                Ok(json!({"type":"function","function":function}))
            }
            Self::Custom {
                description,
                format,
                ..
            } if projection.profile.custom_tools == OpenAiChatCustomToolMode::Native => {
                let mut custom = Map::new();
                custom.insert("name".into(), Value::String(chat_name.to_owned()));
                if let Some(description) = description {
                    custom.insert("description".into(), Value::String(description));
                }
                if let Some(format) = format {
                    custom.insert("format".into(), format);
                }
                Ok(json!({"type":"custom","custom":custom}))
            }
            Self::Custom {
                description,
                format,
                ..
            } => {
                validate_envelope_format(format.as_ref())?;
                let mut function = Map::new();
                function.insert("name".into(), Value::String(chat_name.to_owned()));
                if let Some(description) = description {
                    function.insert("description".into(), Value::String(description));
                }
                function.insert(
                    "parameters".into(),
                    json!({"type":"object","properties":{"input":{"type":"string"}},
                        "required":["input"],"additionalProperties":false}),
                );
                function.insert("strict".into(), Value::Bool(true));
                Ok(json!({"type":"function","function":function}))
            }
            Self::ToolSearch {
                description,
                parameters,
            } => Ok(json!({"type":"function","function":{
                "name":chat_name,"description":description,"parameters":parameters,"strict":false
            }})),
        }
    }
}

fn validate_envelope_format(format: Option<&Value>) -> Result<(), ProtocolError> {
    let Some(format) = format else {
        return Ok(());
    };
    let object = format
        .as_object()
        .ok_or_else(|| invalid("custom tool.format must be an object"))?;
    if object.len() == 1 && object.get("type").and_then(Value::as_str) == Some("text") {
        return Ok(());
    }
    Err(ProtocolError::InvalidPayload(
        "custom tool grammar requires native Chat custom tool support".into(),
    ))
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

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
