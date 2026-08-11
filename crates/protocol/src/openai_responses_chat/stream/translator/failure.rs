use serde_json::{Map, Value, json};

use super::super::wire::{SynthesizedEvent, terminal_event};
use crate::{
    ProtocolError,
    api::{ProtocolEventTelemetry, StreamRejection, StreamTermination},
};

pub(super) fn convert(
    value: &Value,
    rejection: Option<StreamRejection>,
) -> Result<SynthesizedEvent, ProtocolError> {
    let source = value
        .get("error")
        .and_then(Value::as_object)
        .or_else(|| value.as_object())
        .ok_or_else(|| invalid("Chat Completions stream error must be an object"))?;
    let message = required_string(source, "message")?;
    let code = optional_string(source, "code")?
        .or(optional_string(source, "type")?.filter(|value| *value != "error"))
        .unwrap_or("upstream_error");
    let param = match source.get("param") {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(value)) => Value::String(value.clone()),
        Some(_) => {
            return Err(invalid(
                "Chat Completions stream error param must be a string or null",
            ));
        }
    };
    Ok(terminal_event(
        "error",
        json!({"type":"error","code":code,"message":message,"param":param}),
        ProtocolEventTelemetry::default(),
        StreamTermination::Failed,
    )
    .with_rejection(rejection))
}

fn required_string<'a>(
    source: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a str, ProtocolError> {
    optional_string(source, field)?
        .ok_or_else(|| invalid("Chat Completions stream error message must be a non-empty string"))
}

fn optional_string<'a>(
    source: &'a Map<String, Value>,
    field: &'static str,
) -> Result<Option<&'a str>, ProtocolError> {
    match source.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value)),
        Some(_) => Err(invalid(
            "Chat Completions stream error fields must be non-empty strings or null",
        )),
    }
}

fn invalid(message: &'static str) -> ProtocolError {
    ProtocolError::InvalidPayload(message.into())
}
