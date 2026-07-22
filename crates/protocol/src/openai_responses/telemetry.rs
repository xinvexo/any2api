use any2api_domain::TokenUsage;
use bytes::Bytes;
use serde_json::Value;

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry},
    sse::json_event,
    telemetry::{event_type, non_empty_string, token_usage},
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
    let token_usage = if matches!(kind, Some("response.completed" | "response.incomplete")) {
        usage(
            value
                .get("response")
                .and_then(|response| response.get("usage")),
        )
    } else {
        TokenUsage::default()
    };
    ProtocolEventTelemetry {
        token_usage,
        has_content_delta: kind.is_some_and(|kind| CONTENT_DELTA_EVENTS.contains(&kind))
            && non_empty_string(value.get("delta")),
    }
}

fn usage(value: Option<&Value>) -> TokenUsage {
    token_usage(
        value,
        &["input_tokens"],
        &["output_tokens"],
        &["input_tokens_details", "cached_tokens"],
        &["input_tokens_details", "cache_write_tokens"],
    )
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;
    use bytes::Bytes;

    use super::{event, response};

    #[test]
    fn extracts_json_and_terminal_event_usage() {
        let expected = TokenUsage::new(Some(12), Some(7), Some(3), Some(2));
        let json = Bytes::from_static(
            br#"{"usage":{"input_tokens":12,"output_tokens":7,"input_tokens_details":{"cached_tokens":3,"cache_write_tokens":2}}}"#,
        );
        assert_eq!(response(&json).token_usage, expected);

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
        assert!(!event(&empty).has_content_delta);
    }

    #[test]
    fn malformed_or_unstorable_usage_is_ignored() {
        let json = Bytes::from_static(
            br#"{"usage":{"input_tokens":11,"output_tokens":"7","input_tokens_details":{"cached_tokens":9007199254740992,"cache_write_tokens":2}}}"#,
        );

        assert_eq!(
            response(&json).token_usage,
            TokenUsage::new(Some(11), None, None, Some(2))
        );
    }
}
