use any2api_domain::TokenUsage;
use serde_json::{Value, json};

use crate::{
    ProtocolError,
    api::{OpenAiChatCachedTokensField, OpenAiChatCompletionsProfile},
};

pub(in crate::openai_responses_chat) fn responses_usage(
    value: Option<&Value>,
    profile: OpenAiChatCompletionsProfile,
) -> Result<Value, ProtocolError> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    if value.is_null() {
        return Ok(Value::Null);
    }
    let usage = value
        .as_object()
        .ok_or_else(|| invalid("Chat Completions usage must be an object or null"))?;
    let prompt = required_token(usage.get("prompt_tokens"), "usage.prompt_tokens")?;
    let completion = required_token(usage.get("completion_tokens"), "usage.completion_tokens")?;
    let cached = cached_tokens(value, profile)?.unwrap_or(0);
    let reasoning =
        optional_token(value, &["completion_tokens_details", "reasoning_tokens"])?.unwrap_or(0);
    Ok(json!({
        "input_tokens":prompt,"output_tokens":completion,
        "total_tokens":prompt.saturating_add(completion),
        "input_tokens_details":{"cached_tokens":cached},
        "output_tokens_details":{"reasoning_tokens":reasoning}
    }))
}

pub(in crate::openai_responses_chat) fn token_usage(
    value: Option<&Value>,
    profile: OpenAiChatCompletionsProfile,
) -> Result<TokenUsage, ProtocolError> {
    Ok(TokenUsage::new(
        token(value, &["prompt_tokens"]),
        token(value, &["completion_tokens"]),
        value
            .map(|value| cached_tokens(value, profile))
            .transpose()?
            .flatten(),
    ))
}

fn cached_tokens(
    value: &Value,
    profile: OpenAiChatCompletionsProfile,
) -> Result<Option<u64>, ProtocolError> {
    let nested = || optional_token(value, &["prompt_tokens_details", "cached_tokens"]);
    let top_level = || optional_token(value, &["cached_tokens"]);
    match profile.cached_tokens_field {
        OpenAiChatCachedTokensField::PromptTokensDetails => nested(),
        OpenAiChatCachedTokensField::TopLevel => top_level(),
        OpenAiChatCachedTokensField::PromptTokensDetailsOrTopLevel => {
            match (nested()?, top_level()?) {
                (Some(left), Some(right)) if left != right => Err(invalid(
                    "Chat Completions usage contains conflicting cached token counts",
                )),
                (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
                (None, None) => Ok(None),
            }
        }
    }
}

fn required_token(value: Option<&Value>, field: &'static str) -> Result<u64, ProtocolError> {
    value
        .and_then(Value::as_u64)
        .ok_or_else(|| ProtocolError::InvalidPayload(format!("{field} must be an integer")))
}

fn optional_token(value: &Value, path: &[&str]) -> Result<Option<u64>, ProtocolError> {
    let mut current = Some(value);
    for part in path {
        current = current.and_then(|value| value.get(*part));
    }
    match current {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid("Chat Completions usage token counts must be integers")),
    }
}

fn token(mut value: Option<&Value>, path: &[&str]) -> Option<u64> {
    for part in path {
        value = value?.get(*part);
    }
    value?.as_u64()
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
