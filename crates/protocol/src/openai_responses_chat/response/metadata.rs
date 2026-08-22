use serde_json::Value;

use crate::ProtocolError;

pub(super) fn created_at(value: Option<&Value>, fallback: u64) -> Result<u64, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(fallback),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| invalid("Chat Completions created must be a non-negative integer")),
    }
}

pub(super) fn response_model(
    value: Option<&Value>,
    fallback: &str,
) -> Result<String, ProtocolError> {
    match value {
        None | Some(Value::Null) => Ok(fallback.to_owned()),
        Some(Value::String(value)) if !value.is_empty() => Ok(value.clone()),
        Some(_) => Err(invalid("Chat Completions model must be a non-empty string")),
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
