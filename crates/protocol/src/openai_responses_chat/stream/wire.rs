use any2api_domain::TokenUsage;
use bytes::Bytes;
use serde_json::Value;

use crate::api::{
    AdapterEvent, ProtocolEventTelemetry, SseEventPayload, SseJsonData, StreamRejection,
    StreamTermination,
};

pub(super) struct SynthesizedEvent {
    kind: &'static str,
    data: Value,
    telemetry: ProtocolEventTelemetry,
    termination: StreamTermination,
    rejection: Option<StreamRejection>,
}

impl SynthesizedEvent {
    pub(super) fn data_mut(&mut self) -> &mut Value {
        &mut self.data
    }

    pub(super) fn with_rejection(mut self, rejection: Option<StreamRejection>) -> Self {
        self.rejection = rejection;
        self
    }
}

pub(super) fn event_default(kind: &'static str, data: Value) -> SynthesizedEvent {
    event(kind, data, ProtocolEventTelemetry::default())
}

pub(super) fn event(
    kind: &'static str,
    data: Value,
    telemetry: ProtocolEventTelemetry,
) -> SynthesizedEvent {
    SynthesizedEvent {
        kind,
        data,
        telemetry,
        termination: StreamTermination::None,
        rejection: None,
    }
}

pub(super) fn terminal_event(
    kind: &'static str,
    data: Value,
    telemetry: ProtocolEventTelemetry,
    termination: StreamTermination,
) -> SynthesizedEvent {
    SynthesizedEvent {
        kind,
        data,
        telemetry,
        termination,
        rejection: None,
    }
}

pub(super) fn encode_event(event: SynthesizedEvent) -> AdapterEvent {
    #[cfg(test)]
    record_encoding();
    let SynthesizedEvent {
        kind,
        data,
        telemetry,
        termination,
        rejection,
    } = event;
    let encoded = serde_json::to_string(&data).expect("JSON value encodes");
    let prefix = "event: ".len() + kind.len() + "\ndata: ".len();
    let bytes = Bytes::from(format!("event: {kind}\ndata: {encoded}\n\n"));
    let payload = SseEventPayload::Json(SseJsonData::new(
        Some(Bytes::from_static(kind.as_bytes())),
        bytes.slice(prefix..prefix + encoded.len()),
    ));
    AdapterEvent::new(bytes, telemetry, payload)
        .with_termination(termination)
        .with_rejection(rejection)
}

pub(super) fn content_telemetry() -> ProtocolEventTelemetry {
    ProtocolEventTelemetry {
        token_usage: TokenUsage::default(),
        has_content_delta: true,
        retry_transparent: false,
    }
}

#[cfg(test)]
std::thread_local! {
    static ENCODING_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn record_encoding() {
    ENCODING_COUNT.set(ENCODING_COUNT.get() + 1);
}

#[cfg(test)]
pub(super) fn reset_encoding_count() {
    ENCODING_COUNT.set(0);
}

#[cfg(test)]
pub(super) fn encoding_count() -> usize {
    ENCODING_COUNT.get()
}
