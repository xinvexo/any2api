use any2api_domain::TokenUsage;
use serde_json::{Value, value::RawValue};

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    raw_json::{object_field_raw, top_fields},
    telemetry::{raw_event_type, raw_non_empty_string, raw_token_usage, token_usage},
};

const CONTENT_DELTA_EVENTS: &[&str] = &[
    "response.output_text.delta",
    "response.refusal.delta",
    "response.reasoning_text.delta",
    "response.reasoning_summary_text.delta",
    "response.function_call_arguments.delta",
    "response.mcp_call_arguments.delta",
    "response.custom_tool_call_input.delta",
    "response.code_interpreter_call_code.delta",
    "response.audio.transcript.delta",
];

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: token_usage(
            value.get("usage"),
            &["input_tokens"],
            &["output_tokens"],
            &["input_tokens_details", "cached_tokens"],
        ),
    }
}

pub(super) fn raw_response(body: &[u8]) -> ProtocolResponseTelemetry {
    let [usage_field] = top_fields(body, ["usage"]);
    ProtocolResponseTelemetry {
        token_usage: usage(usage_field),
    }
}

pub(super) fn event(payload: &SseEventPayload) -> ProtocolEventTelemetry {
    let SseEventPayload::Json(data) = payload else {
        return ProtocolEventTelemetry {
            retry_transparent: matches!(payload, SseEventPayload::Empty),
            ..ProtocolEventTelemetry::default()
        };
    };
    let [kind, response, delta] = top_fields(data.data(), ["type", "response", "delta"]);
    let kind = raw_event_type(data.event_name(), kind);
    let token_usage = if matches!(
        kind.as_deref(),
        Some("response.completed" | "response.incomplete")
    ) {
        usage(response.and_then(|response| object_field_raw(response.get().as_bytes(), "usage")))
    } else {
        TokenUsage::default()
    };
    ProtocolEventTelemetry {
        token_usage,
        has_content_delta: kind
            .as_deref()
            .is_some_and(|kind| CONTENT_DELTA_EVENTS.contains(&kind))
            && raw_non_empty_string(delta),
        retry_transparent: matches!(
            kind.as_deref(),
            Some("response.created" | "response.in_progress" | "response.queued" | "ping")
        ),
    }
}

fn usage(value: Option<&RawValue>) -> TokenUsage {
    raw_token_usage(
        value,
        &["input_tokens"],
        &["output_tokens"],
        &["input_tokens_details", "cached_tokens"],
    )
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;
    use bytes::Bytes;

    use crate::{api::ProtocolEventTelemetry, sse::parse_event_payload};

    fn event(bytes: &Bytes) -> ProtocolEventTelemetry {
        super::event(&parse_event_payload(bytes).expect("event payload"))
    }

    fn response(body: &[u8]) -> crate::api::ProtocolResponseTelemetry {
        let structured = super::response(&serde_json::from_slice(body).expect("response JSON"));
        assert_eq!(super::raw_response(body), structured);
        structured
    }

    #[test]
    fn extracts_json_and_terminal_event_usage() {
        let expected = TokenUsage::new(Some(12), Some(7), Some(3));
        let json = br#"{"usage":{"input_tokens":12,"output_tokens":7,"input_tokens_details":{"cached_tokens":3,"cache_write_tokens":2}}}"#;
        assert_eq!(response(json).token_usage, expected);

        let sse = Bytes::from_static(
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":12,\"output_tokens\":7,\"input_tokens_details\":{\"cached_tokens\":3,\"cache_write_tokens\":2}}}}\n\n",
        );
        assert_eq!(event(&sse).token_usage, expected);
    }

    #[test]
    fn recognizes_only_non_empty_model_output_deltas() {
        let content = Bytes::from_static(
            b"event: response.function_call_arguments.delta\ndata: {\"type\":\"response.function_call_arguments.delta\",\"delta\":\"{\"}\n\n",
        );
        let control = Bytes::from_static(
            b"event: response.created\ndata: {\"type\":\"response.created\"}\n\n",
        );
        let empty = Bytes::from_static(
            b"event: response.output_text.delta\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"\"}\n\n",
        );

        assert!(event(&content).has_content_delta);
        assert!(!event(&control).has_content_delta);
        assert!(event(&control).retry_transparent);
        assert!(!event(&empty).has_content_delta);
    }

    #[test]
    fn malformed_or_unstorable_usage_is_ignored() {
        let json = br#"{"usage":{"input_tokens":11,"output_tokens":"7","input_tokens_details":{"cached_tokens":9007199254740992,"cache_write_tokens":2}}}"#;

        assert_eq!(
            response(json).token_usage,
            TokenUsage::new(Some(11), None, None)
        );
    }
}
