use any2api_domain::OpenAiChatReasoningResponse;
use serde_json::Value;

use super::{super::wire::SynthesizedEvent, ChatToResponsesStream};
use crate::{ProtocolError, openai_responses_chat::response::validate_finish_reason};

pub(super) fn observe_metadata(
    stream: &mut ChatToResponsesStream,
    value: &Value,
) -> Result<(), ProtocolError> {
    if let Some(object) = value.get("object")
        && object.as_str() != Some("chat.completion.chunk")
    {
        return Err(invalid(
            "Chat Completions stream object must be chat.completion.chunk",
        ));
    }
    if let Some(created) = value.get("created") {
        let created = created.as_u64().ok_or_else(|| {
            invalid("Chat Completions stream created must be a non-negative integer")
        })?;
        if stream
            .observed_created_at
            .is_some_and(|observed| observed != created)
        {
            return Err(invalid("Chat Completions stream created value changed"));
        }
        stream.observed_created_at = Some(created);
        stream.created_at = created;
    }
    if let Some(model) = value.get("model") {
        let model = model
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid("Chat Completions stream model must be a non-empty string"))?;
        if stream
            .observed_model
            .as_deref()
            .is_some_and(|observed| observed != model)
        {
            return Err(invalid("Chat Completions stream model changed"));
        }
        stream.observed_model = Some(model.to_owned());
        stream.model = model.to_owned();
    }
    Ok(())
}

pub(super) fn apply_choices(
    stream: &mut ChatToResponsesStream,
    choices: &[Value],
    events: &mut Vec<SynthesizedEvent>,
) -> Result<(), ProtocolError> {
    if choices.len() > 1 {
        return Err(invalid(
            "Chat Completions stream event must contain at most one choice",
        ));
    }
    let Some(choice) = choices.first() else {
        return Ok(());
    };
    if stream.finish_reason.is_some() {
        return Err(invalid(
            "Chat Completions stream emitted a choice after finish_reason",
        ));
    }
    let choice = choice
        .as_object()
        .ok_or_else(|| invalid("Chat Completions stream choice must be an object"))?;
    if choice.get("index").and_then(Value::as_u64) != Some(0) {
        return Err(invalid("Chat Completions stream choice index must be 0"));
    }
    let delta = choice
        .get("delta")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Chat Completions stream choice delta must be an object"))?;
    validate_role(stream, delta.get("role"))?;
    for field in ["function_call", "refusal", "audio"] {
        if delta.get(field).is_some_and(|value| !value.is_null()) {
            return Err(invalid(
                "Chat Completions stream contains an unsupported output",
            ));
        }
    }
    if let Some(reasoning) = reasoning_delta(stream, delta)? {
        events.extend(stream.push_reasoning(reasoning));
    }
    match delta.get("content") {
        None | Some(Value::Null) => {}
        Some(Value::String(content)) => events.extend(stream.push_text(content)),
        Some(_) => {
            return Err(invalid(
                "streamed assistant content must be a string or null",
            ));
        }
    }
    match delta.get("tool_calls") {
        None | Some(Value::Null) => {}
        Some(Value::Array(calls)) => {
            for call in calls {
                events.extend(stream.push_tool(call)?);
            }
        }
        Some(_) => return Err(invalid("streamed tool_calls must be an array")),
    }
    match choice.get("finish_reason") {
        None | Some(Value::Null) => {}
        Some(Value::String(reason)) => {
            validate_finish_reason(reason)?;
            stream.finish_reason = Some(reason.clone());
        }
        Some(_) => return Err(invalid("streamed finish_reason must be a string or null")),
    }
    Ok(())
}

fn validate_role(
    stream: &mut ChatToResponsesStream,
    value: Option<&Value>,
) -> Result<(), ProtocolError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.as_str() != Some("assistant") {
        return Err(invalid(
            "Chat Completions stream delta role must be assistant",
        ));
    }
    stream.observed_role = true;
    Ok(())
}

fn reasoning_delta<'a>(
    stream: &ChatToResponsesStream,
    delta: &'a serde_json::Map<String, Value>,
) -> Result<Option<&'a str>, ProtocolError> {
    let content = optional_string(delta.get("reasoning_content"))?;
    let reasoning = optional_string(delta.get("reasoning"))?;
    match stream.profile.reasoning_response {
        OpenAiChatReasoningResponse::Unsupported if content.is_some() || reasoning.is_some() => {
            Err(invalid(
                "Chat target returned unsupported reasoning content",
            ))
        }
        OpenAiChatReasoningResponse::Unsupported => Ok(None),
        OpenAiChatReasoningResponse::ReasoningContent if reasoning.is_some() => Err(invalid(
            "Chat target returned an undeclared reasoning field",
        )),
        OpenAiChatReasoningResponse::ReasoningContent => Ok(content),
        OpenAiChatReasoningResponse::Reasoning if content.is_some() => Err(invalid(
            "Chat target returned an undeclared reasoning field",
        )),
        OpenAiChatReasoningResponse::Reasoning => Ok(reasoning),
        OpenAiChatReasoningResponse::ReasoningContentOrReasoning => match (content, reasoning) {
            (Some(left), Some(right)) if left != right => {
                Err(invalid("Chat target returned conflicting reasoning deltas"))
            }
            (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
            (None, None) => Ok(None),
        },
    }
}

fn optional_string(value: Option<&Value>) -> Result<Option<&str>, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(invalid("streamed reasoning content must be a string")),
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
