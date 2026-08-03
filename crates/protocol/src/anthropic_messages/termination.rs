use serde_json::Value;

use crate::api::{SseEventPayload, StreamTermination};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    let SseEventPayload::Json { event_name, data } = payload else {
        return StreamTermination::None;
    };
    if matches_kind(event_name.as_deref(), data, "error") {
        StreamTermination::Failed
    } else if matches_kind(event_name.as_deref(), data, "message_stop") {
        StreamTermination::Completed
    } else {
        StreamTermination::None
    }
}

fn matches_kind(event_name: Option<&str>, data: &Value, expected: &str) -> bool {
    event_name == Some(expected) || data.get("type").and_then(Value::as_str) == Some(expected)
}
