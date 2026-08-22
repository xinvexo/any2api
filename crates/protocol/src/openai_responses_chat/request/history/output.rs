use any2api_domain::{OpenAiChatCompletionsProfile, OpenAiChatReasoningResponse};
use serde_json::{Map, Value, json};

use crate::{ProtocolError, openai_responses_chat::tool_projection::ToolIdentity};

use super::{invalid, required_string};

pub(super) fn insert_reasoning(
    message: &mut Value,
    reasoning: String,
    profile: OpenAiChatCompletionsProfile,
) -> Result<(), ProtocolError> {
    let field = match profile.reasoning_response {
        OpenAiChatReasoningResponse::ReasoningContent
        | OpenAiChatReasoningResponse::ReasoningContentOrReasoning => "reasoning_content",
        OpenAiChatReasoningResponse::Reasoning => "reasoning",
        OpenAiChatReasoningResponse::Unsupported => {
            return Err(invalid("Chat target cannot represent reasoning history"));
        }
    };
    message[field] = Value::String(reasoning);
    Ok(())
}

pub(super) fn validate_identity(
    object: &Map<String, Value>,
    identity: &ToolIdentity,
) -> Result<(), ProtocolError> {
    let Some(name) = object.get("name") else {
        return Ok(());
    };
    let expected = match identity {
        ToolIdentity::Custom { name } | ToolIdentity::NamespaceCustom { name, .. } => name,
        _ => return Err(invalid("name is only valid on custom_tool_call_output")),
    };
    if name.as_str() != Some(expected) {
        return Err(invalid(
            "custom_tool_call_output.name does not match its call",
        ));
    }
    Ok(())
}

pub(super) fn tool_search_tools(object: &Map<String, Value>) -> Result<&[Value], ProtocolError> {
    object
        .get("tools")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("tool_search_output.tools must be an array"))
}

pub(super) fn tool_search_output(
    object: &Map<String, Value>,
    tools: &[Value],
) -> Result<String, ProtocolError> {
    let status = required_string(object.get("status"), "tool_search_output.status")?;
    if required_string(object.get("execution"), "tool_search_output.execution")? != "client" {
        return Err(invalid(
            "only client-executed tool_search_output is supported",
        ));
    }
    serde_json::to_string(&json!({"status":status,"execution":"client","tools":tools}))
        .map_err(|_| invalid("tool_search_output could not be encoded"))
}
