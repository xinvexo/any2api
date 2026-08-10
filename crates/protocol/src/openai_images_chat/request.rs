use serde_json::{Map, Value, json};

use crate::{ProtocolError, api::BridgeRequestFieldBehavior};

use super::capabilities::CAPABILITIES;

pub(super) struct ConvertedRequest {
    pub(super) body: Value,
    pub(super) expected_choices: usize,
}

pub(super) fn convert(
    source: &Value,
    upstream_model: &str,
) -> Result<ConvertedRequest, ProtocolError> {
    let object = source
        .as_object()
        .ok_or_else(|| invalid("Images request body must be a JSON object"))?;
    reject_unknown_fields(object)?;
    reject_streaming(object)?;
    validate_response_format(object)?;
    validate_partial_images(object)?;
    let prompt = object
        .get("prompt")
        .and_then(Value::as_str)
        .filter(|prompt| !prompt.trim().is_empty())
        .ok_or_else(|| invalid("Images prompt must be a non-empty string"))?;
    let expected_choices = expected_choices(object)?;

    let mut chat = Map::new();
    chat.insert("model".into(), Value::String(upstream_model.to_owned()));
    chat.insert("messages".into(), json!([{"role":"user","content":prompt}]));
    chat.insert("stream".into(), Value::Bool(false));
    for field in CAPABILITIES
        .request_fields
        .iter()
        .filter(|field| field.behavior == BridgeRequestFieldBehavior::Forwarded)
    {
        if let Some(value) = object.get(field.path) {
            chat.insert(field.path.into(), value.clone());
        }
    }

    Ok(ConvertedRequest {
        body: Value::Object(chat),
        expected_choices,
    })
}

fn reject_unknown_fields(object: &Map<String, Value>) -> Result<(), ProtocolError> {
    if let Some(field) = object
        .keys()
        .find(|field| CAPABILITIES.request_field(field).is_none())
    {
        return Err(ProtocolError::unsupported_field(None, field));
    }
    Ok(())
}

fn reject_streaming(object: &Map<String, Value>) -> Result<(), ProtocolError> {
    match object.get("stream") {
        None | Some(Value::Bool(false)) => Ok(()),
        Some(Value::Bool(true)) => Err(invalid(
            "Images to Chat Completions bridge does not support streaming",
        )),
        Some(_) => Err(invalid("Images stream must be a boolean")),
    }
}

fn validate_response_format(object: &Map<String, Value>) -> Result<(), ProtocolError> {
    match object.get("response_format") {
        None => Ok(()),
        Some(Value::String(format)) if format == "url" => Ok(()),
        Some(_) => Err(invalid(
            "Images to Chat Completions bridge only supports URL responses",
        )),
    }
}

fn validate_partial_images(object: &Map<String, Value>) -> Result<(), ProtocolError> {
    match object.get("partial_images") {
        None => Ok(()),
        Some(value) if value.as_u64() == Some(0) => Ok(()),
        Some(_) => Err(invalid(
            "Images to Chat Completions bridge does not support partial images",
        )),
    }
}

fn expected_choices(object: &Map<String, Value>) -> Result<usize, ProtocolError> {
    let Some(value) = object.get("n") else {
        return Ok(1);
    };
    let count = value
        .as_u64()
        .filter(|count| (1..=10).contains(count))
        .ok_or_else(|| invalid("Images n must be an integer between 1 and 10"))?;
    usize::try_from(count).map_err(|_| invalid("Images n is too large"))
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
