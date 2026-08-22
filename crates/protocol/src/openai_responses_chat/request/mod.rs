mod history;
mod input;
mod ledger;
mod options;
mod target;

use serde_json::{Map, Value};

use crate::{ProtocolError, api::OpenAiChatCompletionsProfile};

use super::capabilities::CAPABILITIES;
use super::tool_projection::ToolProjection;

pub(super) struct ConvertedRequest {
    pub(super) body: Value,
    conversation_start: usize,
    projection: ToolProjection,
}

impl ConvertedRequest {
    pub(super) fn conversation(&self) -> &[Value] {
        self.body["messages"]
            .as_array()
            .expect("converted request messages are an array")
            .get(self.conversation_start..)
            .expect("conversation start is within converted messages")
    }

    pub(super) fn into_conversation(mut self) -> Vec<Value> {
        let messages = self
            .body
            .as_object_mut()
            .expect("converted request body is an object")
            .remove("messages")
            .expect("converted request messages are an array");
        let Value::Array(mut messages) = messages else {
            unreachable!("converted request messages are an array");
        };
        messages.split_off(self.conversation_start)
    }

    pub(super) fn projection(&self) -> &ToolProjection {
        &self.projection
    }
}

pub(super) fn convert(
    body: &Value,
    upstream_model: &str,
    profile: OpenAiChatCompletionsProfile,
    previous: Vec<Value>,
    previous_projection: Option<&ToolProjection>,
) -> Result<ConvertedRequest, ProtocolError> {
    let source = body
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    validate_top_level_fields(source)?;
    reject_multiple_choices(source)?;
    validate_prompt_cache_key(source.get("prompt_cache_key"))?;
    options::validate_include(source.get("include"))?;
    options::validate_client_metadata(source.get("client_metadata"))?;
    let mut projection = ToolProjection::new(profile, previous_projection);
    projection.configure(source.get("tools"))?;
    let mut conversation = previous;
    input::append_input(
        source.get("input"),
        &mut conversation,
        &mut projection,
        profile,
    )?;
    let mut messages = Vec::with_capacity(conversation.len().saturating_add(1));
    input::append_instructions(source.get("instructions"), &mut messages, profile)?;
    let conversation_start = messages.len();
    messages.extend(conversation);

    let target = target::build(source, upstream_model, profile, messages, &projection)?;

    Ok(ConvertedRequest {
        body: target,
        conversation_start,
        projection,
    })
}

fn validate_top_level_fields(source: &Map<String, Value>) -> Result<(), ProtocolError> {
    if let Some(field) = source
        .keys()
        .find(|field| CAPABILITIES.request_field(field).is_none())
    {
        return Err(ProtocolError::unsupported_field(None, field));
    }
    Ok(())
}

fn reject_multiple_choices(source: &Map<String, Value>) -> Result<(), ProtocolError> {
    if let Some(value) = source.get("n")
        && value.as_u64() != Some(1)
    {
        return Err(invalid("Responses bridge supports exactly one choice"));
    }
    Ok(())
}

fn validate_prompt_cache_key(value: Option<&Value>) -> Result<(), ProtocolError> {
    match value {
        None | Some(Value::String(_)) => Ok(()),
        Some(_) => Err(invalid("prompt_cache_key must be a string")),
    }
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
