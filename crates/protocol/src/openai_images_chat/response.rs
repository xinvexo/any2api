use std::collections::BTreeMap;

use http::Uri;
use serde_json::{Map, Value, json};

use crate::ProtocolError;

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn convert(body: Value, expected_choices: usize) -> Result<Value, ProtocolError> {
    let object = body
        .as_object()
        .ok_or_else(|| invalid("Chat Completions response must be a JSON object"))?;
    let created = safe_integer(object.get("created"))
        .ok_or_else(|| invalid("Chat Completions response has no valid created timestamp"))?;
    let choices = object
        .get("choices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Chat Completions response has no choices"))?;
    if choices.len() != expected_choices {
        return Err(invalid(
            "Chat Completions response returned an unexpected number of image choices",
        ));
    }

    let mut images = BTreeMap::new();
    for choice in choices {
        let (index, url) = convert_choice(choice)?;
        if images.insert(index, json!({"url":url})).is_some() {
            return Err(invalid(
                "Chat Completions response contains duplicate choice indices",
            ));
        }
    }

    let mut converted = json!({
        "created": created,
        "data": images.into_values().collect::<Vec<_>>()
    });
    if let Some(usage) = images_usage(object.get("usage")) {
        converted["usage"] = usage;
    }
    Ok(converted)
}

fn convert_choice(choice: &Value) -> Result<(u64, String), ProtocolError> {
    let choice = choice
        .as_object()
        .ok_or_else(|| invalid("Chat Completions choice must be an object"))?;
    let index = safe_integer(choice.get("index"))
        .ok_or_else(|| invalid("Chat Completions choice has no valid index"))?;
    if choice.get("finish_reason").and_then(Value::as_str) != Some("stop") {
        return Err(invalid(
            "Chat Completions image choice did not finish normally",
        ));
    }
    let message = choice
        .get("message")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Chat Completions image choice has no message"))?;
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Err(invalid(
            "Chat Completions image message role must be assistant",
        ));
    }
    let content = message
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Chat Completions image message content must be a string"))?;
    Ok((index, image_url(content)?))
}

fn image_url(content: &str) -> Result<String, ProtocolError> {
    let content = content.trim();
    if valid_http_url(content) {
        return Ok(content.to_owned());
    }
    let Some(after_alt) = content.strip_prefix("![") else {
        return Err(invalid(
            "Chat Completions image message did not contain an image URL",
        ));
    };
    let Some(alt_end) = after_alt.find("](") else {
        return Err(invalid(
            "Chat Completions image message contained invalid Markdown",
        ));
    };
    let url_with_close = &after_alt[alt_end + 2..];
    let Some(url) = url_with_close.strip_suffix(')') else {
        return Err(invalid(
            "Chat Completions image message contained invalid Markdown",
        ));
    };
    let url = url.trim();
    if !valid_http_url(url) {
        return Err(invalid(
            "Chat Completions image message URL must use HTTP or HTTPS",
        ));
    }
    Ok(url.to_owned())
}

fn valid_http_url(value: &str) -> bool {
    value.parse::<Uri>().is_ok_and(|uri| {
        matches!(uri.scheme_str(), Some("http" | "https")) && uri.authority().is_some()
    })
}

fn images_usage(value: Option<&Value>) -> Option<Value> {
    let usage = value?.as_object()?;
    let input = token(usage, "prompt_tokens").or_else(|| token(usage, "input_tokens"))?;
    let output = token(usage, "completion_tokens").or_else(|| token(usage, "output_tokens"))?;
    let total = token(usage, "total_tokens").or_else(|| {
        input
            .checked_add(output)
            .filter(|total| *total <= MAX_SAFE_INTEGER)
    })?;
    Some(json!({
        "total_tokens": total,
        "input_tokens": input,
        "output_tokens": output
    }))
}

fn token(object: &Map<String, Value>, field: &str) -> Option<u64> {
    safe_integer(object.get(field))
}

fn safe_integer(value: Option<&Value>) -> Option<u64> {
    value?.as_u64().filter(|value| *value <= MAX_SAFE_INTEGER)
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
