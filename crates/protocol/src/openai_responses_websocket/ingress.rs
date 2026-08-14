//! Parses `response.create` messages from the Responses WebSocket ingress.

use serde_json::{Map, Value};

use crate::ProtocolError;

pub(crate) const TURN_STATE_METADATA_KEY: &str = "x-codex-turn-state";
const STREAM_REQUEST_START_METADATA_KEY: &str = "x-codex-ws-stream-request-start-ms";
const SESSION_ID_METADATA_KEY: &str = "session_id";
const THREAD_ID_METADATA_KEY: &str = "thread_id";

/// One `response.create` message split into the transport-independent request
/// object and the WebSocket-only continuation controls.
pub struct ResponsesWsIngress {
    payload: Map<String, Value>,
    input: Vec<Value>,
    previous_response_id: Option<String>,
    warmup: bool,
    turn_state: Option<String>,
    session_id: Option<String>,
    thread_id: Option<String>,
}

impl ResponsesWsIngress {
    /// Splits a WebSocket text message into the HTTP-equivalent request payload
    /// and the transport controls (`type`, `previous_response_id`, `generate`,
    /// and the WebSocket-only `client_metadata` keys are removed).
    pub fn parse(text: &str) -> Result<Self, ProtocolError> {
        let Value::Object(mut payload) = serde_json::from_str::<Value>(text).map_err(|_| {
            ProtocolError::InvalidPayload("websocket message is not valid JSON".into())
        })?
        else {
            return Err(ProtocolError::InvalidPayload(
                "websocket message must be a JSON object".into(),
            ));
        };
        match payload.remove("type") {
            Some(Value::String(kind)) if kind == "response.create" => {}
            _ => {
                return Err(ProtocolError::InvalidPayload(
                    "websocket request type must be response.create".into(),
                ));
            }
        }
        if payload.get("stream") != Some(&Value::Bool(true)) {
            return Err(ProtocolError::InvalidPayload(
                "websocket requests must set stream to true".into(),
            ));
        }
        let previous_response_id = match payload.remove("previous_response_id") {
            None | Some(Value::Null) => None,
            Some(Value::String(id)) if !id.trim().is_empty() => Some(id),
            Some(_) => {
                return Err(ProtocolError::InvalidPayload(
                    "previous_response_id must be a non-empty string or null".into(),
                ));
            }
        };
        let warmup = match payload.remove("generate") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(generate)) => !generate,
            Some(_) => {
                return Err(ProtocolError::InvalidPayload(
                    "generate must be a boolean or null".into(),
                ));
            }
        };
        let input = match payload.remove("input") {
            None => Vec::new(),
            Some(Value::Array(items)) => items,
            Some(_) => {
                return Err(ProtocolError::InvalidPayload(
                    "input must be an array".into(),
                ));
            }
        };
        let metadata = split_client_metadata(&mut payload)?;
        Ok(Self {
            payload,
            input,
            previous_response_id,
            warmup,
            turn_state: metadata.turn_state,
            session_id: metadata.session_id,
            thread_id: metadata.thread_id,
        })
    }

    /// The request object without `input`; `resolve` re-inserts the effective
    /// input before encoding the upstream-facing body.
    pub(super) fn payload(&self) -> &Map<String, Value> {
        &self.payload
    }

    pub(super) fn input(&self) -> &[Value] {
        &self.input
    }

    #[must_use]
    pub fn previous_response_id(&self) -> Option<&str> {
        self.previous_response_id.as_deref()
    }

    #[must_use]
    pub const fn is_warmup(&self) -> bool {
        self.warmup
    }

    #[must_use]
    pub fn turn_state(&self) -> Option<&str> {
        self.turn_state.as_deref()
    }

    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    #[must_use]
    pub fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }
}

#[derive(Default)]
struct ClientMetadataSplit {
    turn_state: Option<String>,
    session_id: Option<String>,
    thread_id: Option<String>,
}

/// Removes WebSocket transport keys from `client_metadata` and surfaces the
/// session identifiers the HTTP ingress would have received as headers.
fn split_client_metadata(
    payload: &mut Map<String, Value>,
) -> Result<ClientMetadataSplit, ProtocolError> {
    let Some(metadata) = payload.get_mut("client_metadata") else {
        return Ok(ClientMetadataSplit::default());
    };
    let Value::Object(metadata) = metadata else {
        return Err(ProtocolError::InvalidPayload(
            "client_metadata must be an object".into(),
        ));
    };
    let turn_state = match metadata.remove(TURN_STATE_METADATA_KEY) {
        None => None,
        Some(Value::String(value)) if !value.is_empty() => Some(value),
        Some(_) => {
            return Err(ProtocolError::InvalidPayload(
                "client_metadata turn state must be a non-empty string".into(),
            ));
        }
    };
    metadata.remove(STREAM_REQUEST_START_METADATA_KEY);
    let session_id = non_empty_string(metadata.get(SESSION_ID_METADATA_KEY));
    let thread_id = non_empty_string(metadata.get(THREAD_ID_METADATA_KEY));
    if metadata.is_empty() {
        payload.remove("client_metadata");
    }
    Ok(ClientMetadataSplit {
        turn_state,
        session_id,
        thread_id,
    })
}

fn non_empty_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}
