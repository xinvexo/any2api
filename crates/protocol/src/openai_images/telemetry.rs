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
