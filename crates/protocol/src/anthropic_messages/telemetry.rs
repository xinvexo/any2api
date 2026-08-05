use any2api_domain::TokenUsage;
use serde_json::{Value, value::RawValue};

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    raw_json::{json_string, object_field_raw, top_fields},
    telemetry::{raw_event_type, raw_non_empty_string, raw_token_usage, token_usage},
};

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: token_usage(
            value.get("usage"),
            &["input_tokens"],
            &["output_tokens"],
            &["cache_read_input_tokens"],
        ),
    }
}

pub(super) fn event(payload: &SseEventPayload) -> ProtocolEventTelemetry {
    let SseEventPayload::Json(data) = payload else {
        return ProtocolEventTelemetry {
            retry_transparent: matches!(payload, SseEventPayload::Empty),
            ..ProtocolEventTelemetry::default()
        };
    };
    let [kind, message, usage_field, delta] =
        top_fields(data.data(), ["type", "message", "usage", "delta"]);
    let kind = raw_event_type(data.event_name(), kind);
    let token_usage = match kind.as_deref() {
        Some("message_start") => {
            usage(message.and_then(|message| object_field_raw(message.get().as_bytes(), "usage")))
        }
        Some("message_delta") => usage(usage_field),
        _ => TokenUsage::default(),
    };
    ProtocolEventTelemetry {
        token_usage,
        has_content_delta: kind.as_deref() == Some("content_block_delta") && content_delta(delta),
        retry_transparent: matches!(kind.as_deref(), Some("message_start" | "ping")),
    }
}

fn usage(value: Option<&RawValue>) -> TokenUsage {
    raw_token_usage(
        value,
        &["input_tokens"],
        &["output_tokens"],
        &["cache_read_input_tokens"],
    )
}

fn content_delta(delta: Option<&RawValue>) -> bool {
    let Some(delta) = delta else {
        return false;
    };
    let [kind, text, thinking, partial_json] = top_fields(
        delta.get().as_bytes(),
        ["type", "text", "thinking", "partial_json"],
    );
    match kind.and_then(json_string).as_deref() {
        Some("text_delta") => raw_non_empty_string(text),
        Some("thinking_delta") => raw_non_empty_string(thinking),
        Some("input_json_delta") => raw_non_empty_string(partial_json),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;
    use bytes::Bytes;

    use crate::{api::ProtocolEventTelemetry, sse::parse_event_payload};

    fn event(bytes: &Bytes) -> ProtocolEventTelemetry {
        super::event(&parse_event_payload(bytes))
    }

    fn response(body: &[u8]) -> crate::api::ProtocolResponseTelemetry {
        super::response(&serde_json::from_slice(body).expect("response JSON"))
    }

    #[test]
    fn extracts_json_usage_and_cumulative_stream_updates() {
        let json =
            br#"{"usage":{"input_tokens":20,"output_tokens":9,"cache_read_input_tokens":4,"cache_creation_input_tokens":3}}"#;
        assert_eq!(
            response(json).token_usage,
            TokenUsage::new(Some(20), Some(9), Some(4))
        );

        let start = Bytes::from_static(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":1,\"cache_read_input_tokens\":4,\"cache_creation_input_tokens\":3}}}\n\n",
        );
        let delta = Bytes::from_static(
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9}}\n\n",
        );
        assert_eq!(
            event(&start).token_usage,
            TokenUsage::new(Some(20), Some(1), Some(4))
        );
        assert_eq!(
            event(&delta).token_usage,
            TokenUsage::new(None, Some(9), None)
        );
    }

    #[test]
    fn recognizes_text_thinking_and_tool_input_but_not_control_frames() {
        for payload in [
            r#"{"type":"text_delta","text":"hello"}"#,
            r#"{"type":"thinking_delta","thinking":"hmm"}"#,
            r#"{"type":"input_json_delta","partial_json":"{"}"#,
        ] {
            let frame = Bytes::from(format!(
                "event: content_block_delta\ndata: {{\"type\":\"content_block_delta\",\"delta\":{payload}}}\n\n"
            ));
            assert!(event(&frame).has_content_delta);
        }
        let control = Bytes::from_static(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n",
        );
        assert!(!event(&control).has_content_delta);
        assert!(event(&control).retry_transparent);
    }

    #[test]
    fn count_tokens_root_value_is_not_generation_usage() {
        assert_eq!(
            response(br#"{"input_tokens":37}"#).token_usage,
            TokenUsage::default()
        );
    }

    #[test]
    fn malformed_fields_do_not_discard_valid_usage_fields() {
        let body = br#"{"usage":{"input_tokens":15,"output_tokens":-1,"cache_read_input_tokens":3,"cache_creation_input_tokens":9007199254740992}}"#;

        assert_eq!(
            response(body).token_usage,
            TokenUsage::new(Some(15), None, Some(3))
        );
    }
}
