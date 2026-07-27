use any2api_domain::TokenUsage;
use serde_json::Value;

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    telemetry::{event_type, token_usage},
};

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: usage(value.get("usage")),
    }
}

pub(super) fn event(payload: &SseEventPayload) -> ProtocolEventTelemetry {
    let SseEventPayload::Json {
        event_name,
        data: value,
    } = payload
    else {
        return ProtocolEventTelemetry::default();
    };
    let kind = event_type(event_name.as_deref(), value);
    let token_usage = match kind {
        Some("image_generation.completed" | "image_edit.completed") => usage(value.get("usage")),
        _ => TokenUsage::default(),
    };
    ProtocolEventTelemetry {
        token_usage,
        has_content_delta: false,
    }
}

fn usage(value: Option<&Value>) -> TokenUsage {
    token_usage(
        value,
        &["input_tokens"],
        &["output_tokens"],
        &["cache_read_tokens"],
        &["cache_write_tokens"],
    )
}
