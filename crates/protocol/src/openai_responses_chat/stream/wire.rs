use any2api_domain::TokenUsage;
use bytes::Bytes;
use serde_json::Value;

use crate::api::{AdapterEvent, ProtocolEventTelemetry, SseEventPayload};

pub(super) fn sse_default(kind: &str, value: Value) -> AdapterEvent {
    sse(kind, value, ProtocolEventTelemetry::default())
}

pub(super) fn sse(kind: &str, value: Value, telemetry: ProtocolEventTelemetry) -> AdapterEvent {
    let bytes = Bytes::from(format!(
        "event: {kind}\ndata: {}\n\n",
        serde_json::to_string(&value).expect("JSON value encodes")
    ));
    AdapterEvent::new(
        bytes,
        telemetry,
        SseEventPayload::Json {
            event_name: Some(kind.to_owned()),
            data: value,
        },
    )
}

pub(super) fn content_telemetry() -> ProtocolEventTelemetry {
    ProtocolEventTelemetry {
        token_usage: TokenUsage::default(),
        has_content_delta: true,
    }
}
