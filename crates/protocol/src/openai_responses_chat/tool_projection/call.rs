use serde_json::{Map, Value, json};

use crate::ProtocolError;

use super::{ChatToolKind, SourceCallKind, ToolIdentity, ToolProjection};

pub(in crate::openai_responses_chat) struct ProjectedSourceCall {
    pub(in crate::openai_responses_chat) chat_call: Value,
    pub(in crate::openai_responses_chat) identity: ToolIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::openai_responses_chat) enum RestoredToolCall {
    Function {
        name: String,
        namespace: Option<String>,
        arguments: String,
    },
    Custom {
        name: String,
        namespace: Option<String>,
        input: String,
    },
    ToolSearch {
        arguments: Value,
    },
}

pub(super) fn project_source_call(
    projection: &mut ToolProjection,
    object: &Map<String, Value>,
    kind: SourceCallKind,
) -> Result<ProjectedSourceCall, ProtocolError> {
    let identity = source_identity(object, kind)?;
    if kind == SourceCallKind::ToolSearch && !projection.has_registered_identity(&identity) {
        return Err(invalid(
            "tool_search_call requires a registered client tool_search",
        ));
    }
    let chat_name = projection.ensure_identity(identity.clone())?;
    let chat_call = match kind {
        SourceCallKind::Function => {
            let arguments = required_string(object.get("arguments"), "function_call.arguments")?;
            json!({"type":"function","function":{"name":chat_name,"arguments":arguments}})
        }
        SourceCallKind::Custom
            if identity.chat_kind(projection.profile) == ChatToolKind::Custom =>
        {
            let input = required_string_allow_empty(object.get("input"), "custom_tool_call.input")?;
            json!({"type":"custom","custom":{"name":chat_name,"input":input}})
        }
        SourceCallKind::Custom => {
            let input = required_string_allow_empty(object.get("input"), "custom_tool_call.input")?;
            let arguments = serde_json::to_string(&json!({"input":input}))
                .map_err(|_| invalid("custom tool input could not be encoded"))?;
            json!({"type":"function","function":{"name":chat_name,"arguments":arguments}})
        }
        SourceCallKind::ToolSearch => {
            if required_string(object.get("execution"), "tool_search_call.execution")? != "client" {
                return Err(invalid(
                    "only client-executed tool_search_call is supported",
                ));
            }
            let arguments = object
                .get("arguments")
                .ok_or_else(|| invalid("tool_search_call.arguments is required"))?;
            let arguments = serde_json::to_string(arguments)
                .map_err(|_| invalid("tool_search_call.arguments could not be encoded"))?;
            json!({"type":"function","function":{"name":chat_name,"arguments":arguments}})
        }
    };
    Ok(ProjectedSourceCall {
        chat_call,
        identity,
    })
}

pub(super) fn restore_call(
    projection: &ToolProjection,
    kind: ChatToolKind,
    name: &str,
    payload: &str,
) -> Result<RestoredToolCall, ProtocolError> {
    let identity = projection.active_identity(kind, name)?;
    match identity {
        ToolIdentity::Function { name } => Ok(RestoredToolCall::Function {
            name: name.clone(),
            namespace: None,
            arguments: payload.to_owned(),
        }),
        ToolIdentity::NamespaceFunction { namespace, name } => Ok(RestoredToolCall::Function {
            name: name.clone(),
            namespace: Some(namespace.clone()),
            arguments: payload.to_owned(),
        }),
        ToolIdentity::Custom { name } => Ok(RestoredToolCall::Custom {
            name: name.clone(),
            namespace: None,
            input: custom_input(projection, payload)?,
        }),
        ToolIdentity::NamespaceCustom { namespace, name } => Ok(RestoredToolCall::Custom {
            name: name.clone(),
            namespace: Some(namespace.clone()),
            input: custom_input(projection, payload)?,
        }),
        ToolIdentity::ToolSearch => {
            let arguments = serde_json::from_str(payload).map_err(|_| {
                invalid("Chat tool_search arguments must contain one complete JSON value")
            })?;
            Ok(RestoredToolCall::ToolSearch { arguments })
        }
    }
}

fn source_identity(
    object: &Map<String, Value>,
    kind: SourceCallKind,
) -> Result<ToolIdentity, ProtocolError> {
    match kind {
        SourceCallKind::ToolSearch => Ok(ToolIdentity::ToolSearch),
        SourceCallKind::Function | SourceCallKind::Custom => {
            let name_field = if kind == SourceCallKind::Function {
                "function_call.name"
            } else {
                "custom_tool_call.name"
            };
            let name = required_string(object.get("name"), name_field)?.to_owned();
            let namespace = optional_string(object.get("namespace"), "tool call namespace")?;
            Ok(match (kind, namespace) {
                (SourceCallKind::Function, None) => ToolIdentity::Function { name },
                (SourceCallKind::Custom, None) => ToolIdentity::Custom { name },
                (SourceCallKind::Function, Some(namespace)) => {
                    ToolIdentity::NamespaceFunction { namespace, name }
                }
                (SourceCallKind::Custom, Some(namespace)) => {
                    ToolIdentity::NamespaceCustom { namespace, name }
                }
                (SourceCallKind::ToolSearch, _) => unreachable!(),
            })
        }
    }
}

fn custom_input(projection: &ToolProjection, payload: &str) -> Result<String, ProtocolError> {
    if projection.profile.custom_tools == any2api_domain::OpenAiChatCustomToolMode::Native {
        return Ok(payload.to_owned());
    }
    let envelope: Value = serde_json::from_str(payload)
        .map_err(|_| invalid("Chat custom tool envelope must be valid JSON"))?;
    let object = envelope
        .as_object()
        .ok_or_else(|| invalid("Chat custom tool envelope must be an object"))?;
    if object.len() != 1 {
        return Err(invalid(
            "Chat custom tool envelope contains unexpected fields",
        ));
    }
    required_string_allow_empty(object.get("input"), "custom tool envelope input")
        .map(str::to_owned)
}

fn optional_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<String>, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
        Some(_) => Err(invalid(field)),
    }
}

fn required_string<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    required_string_allow_empty(value, field).and_then(|value| {
        (!value.is_empty())
            .then_some(value)
            .ok_or_else(|| invalid(field))
    })
}

fn required_string_allow_empty<'a>(
    value: Option<&'a Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    value.and_then(Value::as_str).ok_or_else(|| invalid(field))
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
