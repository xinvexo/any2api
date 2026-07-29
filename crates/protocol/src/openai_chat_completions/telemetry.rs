use any2api_domain::TokenUsage;
use serde_json::Value;

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    telemetry::{non_empty_string, token_usage},
};

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: usage_from(value.get("usage")),
    }
}

pub(super) fn event(payload: &SseEventPayload) -> ProtocolEventTelemetry {
    let SseEventPayload::Json { data: value, .. } = payload else {
        return ProtocolEventTelemetry::default();
    };
    ProtocolEventTelemetry {
        token_usage: usage_from(value.get("usage")),
        has_content_delta: value
            .get("choices")
            .and_then(Value::as_array)
            .is_some_and(|choices| {
                choices.iter().any(|choice| {
                    choice
                        .get("delta")
                        .and_then(|delta| delta.get("content"))
                        .is_some_and(|content| non_empty_string(Some(content)))
                        || choice
                            .get("delta")
                            .and_then(|delta| delta.get("reasoning_content"))
                            .is_some_and(|content| non_empty_string(Some(content)))
                        || choice
                            .get("delta")
                            .and_then(|delta| delta.get("tool_calls"))
                            .and_then(Value::as_array)
                            .is_some_and(|calls| !calls.is_empty())
                })
            }),
    }
}

fn usage_from(value: Option<&Value>) -> TokenUsage {
    token_usage(
        value,
        &["prompt_tokens"],
        &["completion_tokens"],
        &["prompt_tokens_details", "cached_tokens"],
    )
}
