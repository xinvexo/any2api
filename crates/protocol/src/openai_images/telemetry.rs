use any2api_domain::TokenUsage;
use serde_json::Value;

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    raw_json::top_fields,
    telemetry::{raw_event_type, raw_token_usage, token_usage},
};

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: token_usage(
            value.get("usage"),
            &["input_tokens"],
            &["output_tokens"],
            &["cache_read_tokens"],
        ),
    }
}

pub(super) fn raw_response(body: &[u8]) -> ProtocolResponseTelemetry {
    let [usage] = top_fields(body, ["usage"]);
    ProtocolResponseTelemetry {
        token_usage: raw_token_usage(
            usage,
            &["input_tokens"],
            &["output_tokens"],
            &["cache_read_tokens"],
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
    let [kind, usage] = top_fields(data.data(), ["type", "usage"]);
    let token_usage = match raw_event_type(data.event_name(), kind).as_deref() {
        Some("image_generation.completed" | "image_edit.completed") => raw_token_usage(
            usage,
            &["input_tokens"],
            &["output_tokens"],
            &["cache_read_tokens"],
        ),
        _ => TokenUsage::default(),
    };
    ProtocolEventTelemetry {
        token_usage,
        has_content_delta: false,
        retry_transparent: false,
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;

    #[test]
    fn raw_and_structured_response_usage_match() {
        let body = br#"{"data":[{"b64_json":"large"}],"usage":{"input_tokens":8,"output_tokens":3,"cache_read_tokens":2}}"#;
        let structured = super::response(&serde_json::from_slice(body).expect("response JSON"));

        assert_eq!(super::raw_response(body), structured);
        assert_eq!(
            structured.token_usage,
            TokenUsage::new(Some(8), Some(3), Some(2))
        );
    }
}
