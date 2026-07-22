use any2api_domain::TokenUsage;
use bytes::Bytes;
use serde_json::Value;

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry},
    sse::json_event,
    telemetry::{event_type, non_empty_string, token_usage},
};

pub(super) fn response(body: &Bytes) -> ProtocolResponseTelemetry {
    let usage = serde_json::from_slice::<Value>(body)
        .ok()
        .map(|value| usage(value.get("usage")))
        .unwrap_or_default();
    ProtocolResponseTelemetry { token_usage: usage }
}

pub(super) fn event(bytes: &Bytes) -> ProtocolEventTelemetry {
    let Ok(Some((event_name, value))) = json_event(bytes) else {
        return ProtocolEventTelemetry::default();
    };
    let kind = event_type(event_name.as_deref(), &value);
    let token_usage = match kind {
        Some("message_start") => usage(
            value
                .get("message")
                .and_then(|message| message.get("usage")),
        ),
        Some("message_delta") => usage(value.get("usage")),
        _ => TokenUsage::default(),
    };
    ProtocolEventTelemetry {
        token_usage,
        has_content_delta: kind == Some("content_block_delta") && content_delta(&value),
    }
}

fn usage(value: Option<&Value>) -> TokenUsage {
    token_usage(
        value,
        &["input_tokens"],
        &["output_tokens"],
        &["cache_read_input_tokens"],
        &["cache_creation_input_tokens"],
    )
}

fn content_delta(value: &Value) -> bool {
    let Some(delta) = value.get("delta") else {
        return false;
    };
    match delta.get("type").and_then(Value::as_str) {
        Some("text_delta") => non_empty_string(delta.get("text")),
        Some("thinking_delta") => non_empty_string(delta.get("thinking")),
        Some("input_json_delta") => non_empty_string(delta.get("partial_json")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;
    use bytes::Bytes;

    use super::{event, response};

    #[test]
    fn extracts_json_usage_and_cumulative_stream_updates() {
        let json = Bytes::from_static(
            br#"{"usage":{"input_tokens":20,"output_tokens":9,"cache_read_input_tokens":4,"cache_creation_input_tokens":3}}"#,
        );
        assert_eq!(
            response(&json).token_usage,
            TokenUsage::new(Some(20), Some(9), Some(4), Some(3))
        );

        let start = Bytes::from_static(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":1,\"cache_read_input_tokens\":4,\"cache_creation_input_tokens\":3}}}\n\n",
        );
        let delta = Bytes::from_static(
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9}}\n\n",
        );
        assert_eq!(
            event(&start).token_usage,
            TokenUsage::new(Some(20), Some(1), Some(4), Some(3))
        );
        assert_eq!(
            event(&delta).token_usage,
            TokenUsage::new(None, Some(9), None, None)
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
    }

    #[test]
    fn count_tokens_root_value_is_not_generation_usage() {
        let body = Bytes::from_static(br#"{"input_tokens":37}"#);

        assert_eq!(response(&body).token_usage, TokenUsage::default());
    }

    #[test]
    fn malformed_fields_do_not_discard_valid_usage_fields() {
        let body = Bytes::from_static(
            br#"{"usage":{"input_tokens":15,"output_tokens":-1,"cache_read_input_tokens":3,"cache_creation_input_tokens":9007199254740992}}"#,
        );

        assert_eq!(
            response(&body).token_usage,
            TokenUsage::new(Some(15), None, Some(3), None)
        );
    }
}
