use serde_json::Value;

use crate::api::{SseEventPayload, StreamTermination};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    match payload {
        SseEventPayload::Done => StreamTermination::Completed,
        SseEventPayload::Json { event_name, data } if is_error(event_name.as_deref(), data) => {
            StreamTermination::Failed
        }
        _ => StreamTermination::None,
    }
}

fn is_error(event_name: Option<&str>, data: &Value) -> bool {
    event_name == Some("error")
        || data.get("type").and_then(Value::as_str) == Some("error")
        || data.get("error").is_some_and(Value::is_object)
}
