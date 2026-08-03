use serde_json::Value;

use crate::api::{SseEventPayload, StreamTermination};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    let SseEventPayload::Json { event_name, data } = payload else {
        return StreamTermination::None;
    };
    if is_error(event_name.as_deref(), data) {
        return StreamTermination::Failed;
    }
    if ["image_generation.completed", "image_edit.completed"]
        .into_iter()
        .any(|expected| matches_kind(event_name.as_deref(), data, expected))
    {
        StreamTermination::Completed
    } else {
        StreamTermination::None
    }
}

fn is_error(event_name: Option<&str>, data: &Value) -> bool {
    matches_kind(event_name, data, "error") || data.get("error").is_some_and(Value::is_object)
}

fn matches_kind(event_name: Option<&str>, data: &Value, expected: &str) -> bool {
    event_name == Some(expected) || data.get("type").and_then(Value::as_str) == Some(expected)
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use serde_json::{Value, json};

    use super::classify;
    use crate::{
        OpenAiImagesAdapter,
        api::{ProtocolAdapter, SseEventPayload, StreamCompletionPolicy, StreamTermination},
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
        SseEventPayload::Json {
            event_name: event_name.map(ToOwned::to_owned),
            data,
        }
    }
}
