use any2api_domain::{MAX_TOKEN_COUNT, TokenUsage};
use serde_json::{Value, value::RawValue};

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    raw_json::{json_string, object_field_raw, top_fields},
    telemetry::{raw_event_type, raw_non_empty_string},
};

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: structured_usage(value.get("usage")),
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
    let Some(value) = value else {
        return TokenUsage::default();
    };
    let [input, output, cache_creation, cache_read] = top_fields(
        value.get().as_bytes(),
        [
            "input_tokens",
            "output_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ],
    );
    TokenUsage::new(
        total_input(
            raw_token(input),
            raw_optional_token(cache_creation),
            raw_optional_token(cache_read),
        ),
        raw_token(output),
        raw_token(cache_read),
    )
}

fn structured_usage(value: Option<&Value>) -> TokenUsage {
    let Some(value) = value else {
        return TokenUsage::default();
    };
    let cache_read = value.get("cache_read_input_tokens");
    TokenUsage::new(
        total_input(
            structured_token(value.get("input_tokens")),
            structured_optional_token(value.get("cache_creation_input_tokens")),
            structured_optional_token(cache_read),
        ),
        structured_token(value.get("output_tokens")),
        structured_token(cache_read),
    )
}

fn total_input(
    input: Option<u64>,
    cache_creation: Option<u64>,
    cache_read: Option<u64>,
) -> Option<u64> {
    let total = input?
        .checked_add(cache_creation?)?
        .checked_add(cache_read?)?;
    (total <= MAX_TOKEN_COUNT).then_some(total)
}

fn structured_token(value: Option<&Value>) -> Option<u64> {
    value?.as_u64().filter(|value| *value <= MAX_TOKEN_COUNT)
}

fn structured_optional_token(value: Option<&Value>) -> Option<u64> {
    match value {
        None | Some(Value::Null) => Some(0),
        Some(value) => structured_token(Some(value)),
    }
}

fn raw_token(value: Option<&RawValue>) -> Option<u64> {
    serde_json::from_str::<u64>(value?.get())
        .ok()
        .filter(|value| *value <= MAX_TOKEN_COUNT)
}

fn raw_optional_token(value: Option<&RawValue>) -> Option<u64> {
    let Some(value) = value else {
        return Some(0);
    };
    match serde_json::from_str::<Option<u64>>(value.get()) {
        Ok(None) => Some(0),
        Ok(Some(value)) if value <= MAX_TOKEN_COUNT => Some(value),
        _ => None,
    }
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
        let structured = super::response(&serde_json::from_slice(body).expect("response JSON"));
        assert_eq!(super::raw_response(body), structured);
        structured
    }

    #[test]
    fn extracts_json_usage_and_cumulative_stream_updates() {
        let json =
            br#"{"usage":{"input_tokens":20,"output_tokens":9,"cache_read_input_tokens":4,"cache_creation_input_tokens":3}}"#;
        assert_eq!(
            response(json).token_usage,
            TokenUsage::new(Some(27), Some(9), Some(4))
        );

        let start = Bytes::from_static(
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":20,\"output_tokens\":1,\"cache_read_input_tokens\":4,\"cache_creation_input_tokens\":3}}}\n\n",
        );
        let delta = Bytes::from_static(
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"output_tokens\":9}}\n\n",
        );
        let cached_delta = Bytes::from_static(
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"usage\":{\"input_tokens\":20,\"output_tokens\":9,\"cache_read_input_tokens\":4,\"cache_creation_input_tokens\":3}}\n\n",
        );
        assert_eq!(
            event(&start).token_usage,
            TokenUsage::new(Some(27), Some(1), Some(4))
        );
        assert_eq!(
            event(&delta).token_usage,
            TokenUsage::new(None, Some(9), None)
        );
        assert_eq!(
            event(&cached_delta).token_usage,
            TokenUsage::new(Some(27), Some(9), Some(4))
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
            TokenUsage::new(None, None, Some(3))
        );
    }

    #[test]
    fn absent_or_null_cache_fields_contribute_zero_to_input_total() {
        assert_eq!(
            response(br#"{"usage":{"input_tokens":15,"output_tokens":2}}"#).token_usage,
            TokenUsage::new(Some(15), Some(2), None)
        );
        assert_eq!(
            response(
                br#"{"usage":{"input_tokens":15,"output_tokens":2,"cache_creation_input_tokens":null,"cache_read_input_tokens":null}}"#,
            )
            .token_usage,
            TokenUsage::new(Some(15), Some(2), None)
        );
    }

    #[test]
    fn input_total_above_the_safe_integer_limit_is_unknown() {
        let body = br#"{"usage":{"input_tokens":9007199254740991,"output_tokens":1,"cache_creation_input_tokens":1,"cache_read_input_tokens":0}}"#;

        assert_eq!(
            response(body).token_usage,
            TokenUsage::new(None, Some(1), Some(0))
        );
    }
}
