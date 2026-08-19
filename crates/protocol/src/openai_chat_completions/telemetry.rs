use serde_json::{Value, value::RawValue};

use crate::{
    api::{ProtocolEventTelemetry, ProtocolResponseTelemetry, SseEventPayload},
    raw_json::{raw_array, top_fields},
    telemetry::{raw_non_empty_string, raw_token_usage, token_usage},
};

pub(super) fn response(value: &Value) -> ProtocolResponseTelemetry {
    ProtocolResponseTelemetry {
        token_usage: token_usage(
            value.get("usage"),
            &["prompt_tokens"],
            &["completion_tokens"],
            &["prompt_tokens_details", "cached_tokens"],
        ),
        effective_speed_tier: None,
    }
}

pub(super) fn raw_response(body: &[u8]) -> ProtocolResponseTelemetry {
    let [usage] = top_fields(body, ["usage"]);
    ProtocolResponseTelemetry {
        token_usage: raw_token_usage(
            usage,
            &["prompt_tokens"],
            &["completion_tokens"],
            &["prompt_tokens_details", "cached_tokens"],
        ),
        effective_speed_tier: None,
    }
}

pub(super) fn event(payload: &SseEventPayload) -> ProtocolEventTelemetry {
    let SseEventPayload::Json(data) = payload else {
        return ProtocolEventTelemetry {
            retry_transparent: matches!(payload, SseEventPayload::Empty),
            ..ProtocolEventTelemetry::default()
        };
    };
    let [usage, choices] = top_fields(data.data(), ["usage", "choices"]);
    ProtocolEventTelemetry {
        token_usage: raw_token_usage(
            usage,
            &["prompt_tokens"],
            &["completion_tokens"],
            &["prompt_tokens_details", "cached_tokens"],
        ),
        effective_speed_tier: None,
        has_content_delta: choices
            .and_then(|choices| raw_array(choices.get().as_bytes()))
            .is_some_and(|choices| choices.iter().any(|choice| choice_has_content(choice))),
        retry_transparent: false,
    }
}

fn choice_has_content(choice: &RawValue) -> bool {
    let [delta] = top_fields(choice.get().as_bytes(), ["delta"]);
    let Some(delta) = delta else {
        return false;
    };
    let [content, reasoning_content, tool_calls] = top_fields(
        delta.get().as_bytes(),
        ["content", "reasoning_content", "tool_calls"],
    );
    raw_non_empty_string(content)
        || raw_non_empty_string(reasoning_content)
        || tool_calls
            .and_then(|calls| raw_array(calls.get().as_bytes()))
            .is_some_and(|calls| !calls.is_empty())
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;

    #[test]
    fn raw_and_structured_response_usage_match() {
        let body = br#"{"choices":[{"message":{"content":"large"}}],"usage":{"prompt_tokens":9,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":2}}}"#;
        let structured = super::response(&serde_json::from_slice(body).expect("response JSON"));

        assert_eq!(super::raw_response(body), structured);
        assert_eq!(
            structured.token_usage,
            TokenUsage::new(Some(9), Some(4), Some(2))
        );
    }
}
