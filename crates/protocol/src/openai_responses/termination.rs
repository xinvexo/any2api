use crate::{
    api::{SseEventPayload, StreamTermination},
    raw_json::top_fields,
    telemetry::raw_event_type,
};

pub(super) fn classify(payload: &SseEventPayload) -> StreamTermination {
    let SseEventPayload::Json(data) = payload else {
        return StreamTermination::None;
    };
    let [kind] = top_fields(data.data(), ["type"]);
    match raw_event_type(data.event_name(), kind).as_deref() {
        Some("response.completed" | "response.incomplete") => StreamTermination::Completed,
        Some("response.failed" | "error") => StreamTermination::Failed,
        _ => StreamTermination::None,
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use bytes::Bytes;

    use crate::{
        OpenAiResponsesAdapter,
        api::{ProtocolAdapter, SseFrame, StreamCompletionPolicy, StreamTermination},
    };

    fn decode(kind: &str) -> crate::api::AdapterEvent {
        let frame = SseFrame(Bytes::from(format!(
            "event: {kind}\ndata: {{\"type\":\"{kind}\",\"future_field\":true}}\n\n"
        )));
        OpenAiResponsesAdapter::new()
            .decode_upstream_event(frame)
            .expect("decoded Responses event")
    }

    #[test]
    fn distinguishes_successful_and_failed_terminal_events() {
        for kind in ["response.completed", "response.incomplete"] {
            let event = decode(kind);
            assert_eq!(
                event.termination(),
                StreamTermination::Completed,
                "{kind} must terminate the stream successfully"
            );
            assert!(
                event.upstream_failure().is_none(),
                "{kind} is a successful terminal result"
            );
        }
        for kind in ["response.failed", "error"] {
            let event = decode(kind);
            assert_eq!(
                event.termination(),
                StreamTermination::Failed,
                "{kind} must terminate the stream as a failure"
            );
            assert!(
                event.upstream_failure().is_some(),
                "{kind} must carry complete upstream failure evidence"
            );
        }
        for kind in [
            "response.created",
            "response.output_item.done",
            "compaction_summary",
        ] {
            assert_eq!(
                decode(kind).termination(),
                StreamTermination::None,
                "{kind} must not terminate the stream"
            );
        }
    }

    #[test]
    fn responses_requires_a_terminal_event_but_unary_compact_does_not() {
        let adapter = OpenAiResponsesAdapter::new();
        assert_eq!(
            adapter.stream_completion_policy(ProtocolOperation::Responses),
            StreamCompletionPolicy::TerminalEventRequired
        );
        assert_eq!(
            adapter.stream_completion_policy(ProtocolOperation::ResponsesCompact),
            StreamCompletionPolicy::EofAllowed
        );
    }

    #[test]
    fn compaction_summary_and_unknown_fields_remain_opaque() {
        let bytes = Bytes::from_static(
            b"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"compaction_summary\",\"encrypted_content\":\"opaque\",\"future_item_field\":[1,2]},\"future_event_field\":true}\n\n",
        );
        let adapter = OpenAiResponsesAdapter::new();
        let event = adapter
            .decode_upstream_event(SseFrame(bytes.clone()))
            .expect("decoded compaction event");
        assert!(!event.is_terminal());
        let encoded = adapter
            .encode_egress_event(event, "public")
            .expect("encoded compaction event");
        assert_eq!(encoded.0, bytes);
    }
}
