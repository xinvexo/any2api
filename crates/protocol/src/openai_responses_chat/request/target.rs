use crate::api::{
    OpenAiChatCompletionsProfile, OpenAiChatReasoningRequest, OpenAiChatRequestField,
};
use serde_json::{Map, Value, json};

use crate::{ProtocolError, api::BridgeRequestFieldBehavior};

use super::{ToolProjection, invalid, options};
use crate::openai_responses_chat::capabilities::CAPABILITIES;

pub(super) fn build(
    source: &Map<String, Value>,
    upstream_model: &str,
    profile: OpenAiChatCompletionsProfile,
    messages: Vec<Value>,
    projection: &ToolProjection,
) -> Result<Value, ProtocolError> {
    let mut target = Map::new();
    target.insert("model".into(), Value::String(upstream_model.to_owned()));
    target.insert("messages".into(), Value::Array(messages));
    append_forwarded_fields(source, profile, &mut target)?;
    if let Some(value) = source.get("max_output_tokens") {
        target.insert(profile.token_limit_field.as_str().into(), value.clone());
    }
    if let Some(value) = source.get("stream") {
        target.insert("stream".into(), value.clone());
        if value == &Value::Bool(true) {
            target.insert("stream_options".into(), json!({"include_usage": true}));
        }
    }
    append_reasoning(source, profile, &mut target)?;
    append_text_config(source, profile, &mut target)?;
    append_tools(source, profile, projection, &mut target)?;
    Ok(Value::Object(target))
}

fn append_forwarded_fields(
    source: &Map<String, Value>,
    profile: OpenAiChatCompletionsProfile,
    target: &mut Map<String, Value>,
) -> Result<(), ProtocolError> {
    for field in CAPABILITIES
        .request_fields
        .iter()
        .filter(|field| field.behavior == BridgeRequestFieldBehavior::Forwarded)
    {
        let Some(value) = source.get(field.path) else {
            continue;
        };
        if field.path == "parallel_tool_calls" {
            continue;
        }
        let capability = request_field(field.path).ok_or_else(|| {
            ProtocolError::Internal(format!(
                "forwarded bridge field `{}` has no target profile capability",
                field.path
            ))
        })?;
        if !profile.supports_request_field(capability) {
            return Err(ProtocolError::InvalidPayload(format!(
                "Chat target does not support request field `{}`",
                field.path
            )));
        }
        target.insert(field.path.into(), value.clone());
    }
    Ok(())
}

fn append_reasoning(
    source: &Map<String, Value>,
    profile: OpenAiChatCompletionsProfile,
    target: &mut Map<String, Value>,
) -> Result<(), ProtocolError> {
    let Some(reasoning) = source.get("reasoning") else {
        return Ok(());
    };
    let Some(effort) = options::convert_reasoning(reasoning)? else {
        return Ok(());
    };
    match profile.reasoning_request {
        OpenAiChatReasoningRequest::ReasoningEffort => {
            target.insert("reasoning_effort".into(), effort);
            Ok(())
        }
        OpenAiChatReasoningRequest::Unsupported => Err(ProtocolError::InvalidPayload(
            "Chat target does not support reasoning effort".into(),
        )),
    }
}

fn append_text_config(
    source: &Map<String, Value>,
    profile: OpenAiChatCompletionsProfile,
    target: &mut Map<String, Value>,
) -> Result<(), ProtocolError> {
    let Some(text) = source.get("text") else {
        return Ok(());
    };
    let converted = options::convert_text_config(text)?;
    if let Some(response_format) = converted.response_format {
        require_field(
            profile,
            OpenAiChatRequestField::ResponseFormat,
            "response_format",
        )?;
        target.insert("response_format".into(), response_format);
    }
    if let Some(verbosity) = converted.verbosity {
        require_field(profile, OpenAiChatRequestField::Verbosity, "verbosity")?;
        target.insert("verbosity".into(), verbosity);
    }
    Ok(())
}

fn append_tools(
    source: &Map<String, Value>,
    profile: OpenAiChatCompletionsProfile,
    projection: &ToolProjection,
    target: &mut Map<String, Value>,
) -> Result<(), ProtocolError> {
    if !projection.has_active_tools() {
        return match source.get("tool_choice") {
            None => Ok(()),
            Some(choice) => match choice.as_str() {
                Some("none" | "auto") => Ok(()),
                _ => Err(invalid("tool_choice requires at least one active tool")),
            },
        };
    }
    target.insert(
        "tools".into(),
        Value::Array(projection.chat_tools().to_vec()),
    );
    if let Some(value) = source.get("tool_choice") {
        target.insert("tool_choice".into(), projection.project_tool_choice(value)?);
    }
    if let Some(value) = source.get("parallel_tool_calls") {
        require_field(
            profile,
            OpenAiChatRequestField::ParallelToolCalls,
            "parallel_tool_calls",
        )?;
        target.insert("parallel_tool_calls".into(), value.clone());
    }
    Ok(())
}

fn require_field(
    profile: OpenAiChatCompletionsProfile,
    field: OpenAiChatRequestField,
    name: &'static str,
) -> Result<(), ProtocolError> {
    if profile.supports_request_field(field) {
        Ok(())
    } else {
        Err(ProtocolError::InvalidPayload(format!(
            "Chat target does not support {name}"
        )))
    }
}

fn request_field(path: &str) -> Option<OpenAiChatRequestField> {
    Some(match path {
        "frequency_penalty" => OpenAiChatRequestField::FrequencyPenalty,
        "logit_bias" => OpenAiChatRequestField::LogitBias,
        "logprobs" => OpenAiChatRequestField::Logprobs,
        "metadata" => OpenAiChatRequestField::Metadata,
        "parallel_tool_calls" => OpenAiChatRequestField::ParallelToolCalls,
        "presence_penalty" => OpenAiChatRequestField::PresencePenalty,
        "prompt_cache_key" => OpenAiChatRequestField::PromptCacheKey,
        "seed" => OpenAiChatRequestField::Seed,
        "service_tier" => OpenAiChatRequestField::ServiceTier,
        "stop" => OpenAiChatRequestField::Stop,
        "store" => OpenAiChatRequestField::Store,
        "temperature" => OpenAiChatRequestField::Temperature,
        "top_logprobs" => OpenAiChatRequestField::TopLogprobs,
        "top_p" => OpenAiChatRequestField::TopP,
        "user" => OpenAiChatRequestField::User,
        _ => return None,
    })
}
