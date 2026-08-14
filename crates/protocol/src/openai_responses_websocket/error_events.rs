//! Wrapped `{"type":"error"}` events for the Responses WebSocket ingress.
//!
//! After the upgrade there is no HTTP status channel; failures are delivered
//! as protocol error events carrying the original status, the upstream error
//! object, and the already-sanitized response headers.

use http::{HeaderMap, StatusCode};
use serde_json::{Map, Value, json};

const MAX_ERROR_TEXT_BYTES: usize = 8 * 1024;

/// Wraps a non-2xx bridged response into a WebSocket error event. The `error`
/// object is passed through verbatim when the body is an OpenAI-shaped error.
#[must_use]
pub fn wrapped_error_event(status: StatusCode, headers: &HeaderMap, body: &[u8]) -> String {
    let mut event = Map::new();
    event.insert("type".to_owned(), Value::String("error".to_owned()));
    event.insert("status".to_owned(), Value::from(status.as_u16()));
    event.insert("error".to_owned(), error_object(body));
    let headers = header_object(headers);
    if !headers.is_empty() {
        event.insert("headers".to_owned(), Value::Object(headers));
    }
    Value::Object(event).to_string()
}

/// The protocol-native recovery signal: the client retries with the full
/// request when the connection baseline cannot serve `previous_response_id`.
#[must_use]
pub fn previous_response_not_found_event() -> String {
    json!({
        "type": "error",
        "status": 400,
        "error": {
            "type": "invalid_request_error",
            "code": "previous_response_not_found",
            "message": "Previous response was not found.",
        },
    })
    .to_string()
}

fn error_object(body: &[u8]) -> Value {
    if let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(body) {
        if let Some(error @ Value::Object(_)) = object.get("error") {
            return error.clone();
        }
        if object.contains_key("message") || object.contains_key("code") {
            return Value::Object(object);
        }
    }
    json!({ "message": truncated_text(body) })
}

fn truncated_text(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let mut end = text.len().min(MAX_ERROR_TEXT_BYTES);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

fn header_object(headers: &HeaderMap) -> Map<String, Value> {
    let mut object = Map::new();
    for (name, value) in headers {
        if let Ok(value) = value.to_str() {
            object.insert(name.as_str().to_owned(), Value::String(value.to_owned()));
        }
    }
    object
}
