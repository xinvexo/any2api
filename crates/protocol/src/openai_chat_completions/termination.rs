use serde_json::value::RawValue;

use crate::{
    api::{SseEventPayload, StreamTermination},
    raw_json::{json_string, top_fields},
};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    match payload {
        SseEventPayload::Done => StreamTermination::Completed,
        SseEventPayload::Json(data) => {
            let [kind, error] = top_fields(data.data(), ["type", "error"]);
            if is_error(data.event_name(), kind, error) {
                StreamTermination::Failed
            } else {
                StreamTermination::None
            }
        }
        _ => StreamTermination::None,
    }
}

fn is_error(event_name: Option<&str>, kind: Option<&RawValue>, error: Option<&RawValue>) -> bool {
    event_name == Some("error")
        || kind.and_then(json_string).as_deref() == Some("error")
        || error.is_some_and(|error| error.get().starts_with('{'))
}
