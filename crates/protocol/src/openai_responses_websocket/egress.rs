//! Egress SSE frame classification for WebSocket delivery.

use bytes::Bytes;
use serde_json::Value;

use crate::{
    api::{SseEventPayload, SseFrame},
    raw_json::{json_string, object_field_raw, top_fields},
    sse::parse_event_payload,
};

/// One egress SSE frame prepared for WebSocket delivery: the JSON `data:`
/// payload forwarded verbatim as a text message plus the baseline observation.
pub struct ResponsesWsEgressFrame {
    pub payload: Option<Bytes>,
    pub observation: ResponsesWsObservation,
}

pub enum ResponsesWsObservation {
    None,
    /// A finished output item that extends the next incremental baseline.
    OutputItemDone {
        item: Value,
        raw_len: usize,
    },
    /// The stream completed; the response ID anchors the next baseline.
    Completed {
        response_id: Option<String>,
    },
}

/// Classifies one egress SSE frame. Comment and heartbeat frames yield no
/// WebSocket message; JSON payloads are forwarded without re-encoding.
#[must_use]
pub fn classify_egress_frame(frame: &SseFrame) -> ResponsesWsEgressFrame {
    let SseEventPayload::Json(data) = parse_event_payload(&frame.0) else {
        return ResponsesWsEgressFrame {
            payload: None,
            observation: ResponsesWsObservation::None,
        };
    };
    let bytes = data.data().clone();
    let [kind, item, response] = top_fields(&bytes, ["type", "item", "response"]);
    let kind = kind.and_then(json_string);
    let matches_kind =
        |name: &str| data.event_name() == Some(name) || kind.as_deref() == Some(name);
    let observation = if matches_kind("response.output_item.done") {
        item.and_then(|item| {
            let parsed = serde_json::from_str::<Value>(item.get()).ok()?;
            Some(ResponsesWsObservation::OutputItemDone {
                item: parsed,
                raw_len: item.get().len(),
            })
        })
        .unwrap_or(ResponsesWsObservation::None)
    } else if matches_kind("response.completed") {
        ResponsesWsObservation::Completed {
            response_id: response.and_then(|response| {
                json_string(object_field_raw(response.get().as_bytes(), "id")?)
                    .map(|id| id.into_owned())
            }),
        }
    } else {
        ResponsesWsObservation::None
    };
    ResponsesWsEgressFrame {
        payload: Some(bytes),
        observation,
    }
}

/// Synthetic warmup lifecycle events (`generate: false` is answered locally;
/// the ID lives only in the connection baseline and never reaches upstream).
#[must_use]
pub fn warmup_created_event(response_id: &str) -> String {
    warmup_event("response.created", response_id)
}

#[must_use]
pub fn warmup_completed_event(response_id: &str) -> String {
    warmup_event("response.completed", response_id)
}

fn warmup_event(kind: &str, response_id: &str) -> String {
    serde_json::json!({
        "type": kind,
        "response": {"id": response_id},
    })
    .to_string()
}
