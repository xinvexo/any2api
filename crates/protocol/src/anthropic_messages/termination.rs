use crate::{
    api::{SseEventPayload, StreamTermination},
    raw_json::{json_string, top_fields},
};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    let SseEventPayload::Json(data) = payload else {
        return StreamTermination::None;
    };
    let [kind] = top_fields(data.data(), ["type"]);
    let kind = kind.and_then(json_string);
    let matches =
        |expected: &str| data.event_name() == Some(expected) || kind.as_deref() == Some(expected);
    if matches("error") {
        StreamTermination::Failed
    } else if matches("message_stop") {
        StreamTermination::Completed
    } else {
        StreamTermination::None
    }
}
