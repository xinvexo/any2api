use serde_json::value::RawValue;

use crate::{
    api::{SseEventPayload, StreamTermination},
    raw_json::{json_string, top_fields},
};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    let SseEventPayload::Json(data) = payload else {
        return StreamTermination::None;
    };
    let [kind, error] = top_fields(data.data(), ["type", "error"]);
    let kind = kind.and_then(json_string);
    let matches =
        |expected: &str| data.event_name() == Some(expected) || kind.as_deref() == Some(expected);
    if matches("error") || is_error_object(error) {
        return StreamTermination::Failed;
    }
    if ["image_generation.completed", "image_edit.completed"]
        .into_iter()
        .any(matches)
    {
        StreamTermination::Completed
    } else {
        StreamTermination::None
    }
}

fn is_error_object(error: Option<&RawValue>) -> bool {
    error.is_some_and(|error| error.get().starts_with('{'))
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use bytes::Bytes;
    use serde_json::{Value, json};

    use super::classify;
    use crate::{
        OpenAiImagesAdapter,
        api::{
            ProtocolAdapter, SseEventPayload, SseJsonData, StreamCompletionPolicy,
            StreamTermination,
        },
    };

    #[test]
    fn images_streams_require_and_classify_terminal_events() {
        let adapter = OpenAiImagesAdapter::new();
        for operation in [
            ProtocolOperation::ImagesGenerations,
            ProtocolOperation::ImagesEdits,
        ] {
            assert_eq!(
                adapter.stream_completion_policy(operation),
                StreamCompletionPolicy::TerminalEventRequired
            );
        }

        for kind in ["image_generation.completed", "image_edit.completed"] {
            assert_eq!(
                classify(&json_payload(Some(kind), json!({"type": kind}))),
                StreamTermination::Completed
            );
        }
        assert_eq!(
            classify(&json_payload(
                Some("error"),
                json!({"error": {"message": "generation failed"}}),
            )),
            StreamTermination::Failed
        );
        assert_eq!(
            classify(&json_payload(
                Some("image_generation.partial_image"),
                json!({"type": "image_generation.partial_image"}),
            )),
            StreamTermination::None
        );
    }

    fn json_payload(event_name: Option<&str>, data: Value) -> SseEventPayload {
        SseEventPayload::Json(SseJsonData::new(
            event_name.map(|name| Bytes::from(name.to_owned())),
            Bytes::from(serde_json::to_vec(&data).expect("event JSON")),
        ))
    }
}
