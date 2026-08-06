use crate::{
    api::{SseEventPayload, StreamRetryReason},
    raw_json::{json_string, object_field_raw, top_fields},
    telemetry::raw_event_type,
};

pub(crate) fn openai(payload: &SseEventPayload) -> Option<StreamRetryReason> {
    let SseEventPayload::Json(data) = payload else {
        return None;
    };
    let [kind, error, response, code] =
        top_fields(data.data(), ["type", "error", "response", "code"]);
    let kind = raw_event_type(data.event_name(), kind);
    if !matches!(kind.as_deref(), Some("error" | "response.failed")) && error.is_none() {
        return None;
    }
    let nested_code = error
        .and_then(|error| object_field_raw(error.get().as_bytes(), "code").and_then(json_string));
    let response_code = response
        .and_then(|response| object_field_raw(response.get().as_bytes(), "error"))
        .and_then(|error| object_field_raw(error.get().as_bytes(), "code"))
        .and_then(json_string);
    let top_level_code = code.and_then(json_string);
    [nested_code, response_code, top_level_code]
        .into_iter()
        .flatten()
        .any(|code| code == "server_is_overloaded")
        .then_some(StreamRetryReason::Overloaded)
}

pub(crate) fn anthropic(payload: &SseEventPayload) -> Option<StreamRetryReason> {
    let SseEventPayload::Json(data) = payload else {
        return None;
    };
    let [kind, error] = top_fields(data.data(), ["type", "error"]);
    let event_kind = raw_event_type(data.event_name(), kind);
    if event_kind.as_deref() != Some("error") {
        return None;
    }
    let nested_kind = error
        .and_then(|error| object_field_raw(error.get().as_bytes(), "type"))
        .and_then(json_string);
    // The new rate-limit signal requires the complete Anthropic envelope;
    // keep the existing overload event-name compatibility unchanged.
    match nested_kind.as_deref() {
        Some("overloaded_error") => Some(StreamRetryReason::Overloaded),
        Some("rate_limit_error")
            if kind.and_then(json_string).as_deref() == Some("error")
                && data.event_name().is_none_or(|name| name == "error") =>
        {
            Some(StreamRetryReason::RateLimited)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::{anthropic, openai};
    use crate::{api::StreamRetryReason, sse::parse_event_payload};

    #[test]
    fn recognizes_only_declared_openai_overload_codes() {
        let exact = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"service_unavailable_error\",\"code\":\"server_is_overloaded\",\"message\":\"busy\"}}\n\n",
        ));
        let prose_only = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"code\":\"server_error\",\"message\":\"server is overloaded\"}}\n\n",
        ));
        assert_eq!(openai(&exact), Some(StreamRetryReason::Overloaded));
        assert_eq!(openai(&prose_only), None);
    }

    #[test]
    fn recognizes_only_declared_anthropic_retry_types() {
        let exact = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"busy\"}}\n\n",
        ));
        let event_only_overload = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"error\":{\"type\":\"overloaded_error\"}}\n\n",
        ));
        let unknown = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"overloaded\"}}\n\n",
        ));
        let rate_limited = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\",\"message\":\"Concurrency limit exceeded\"}}\n\n",
        ));
        let prose_only = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"rate_limit_error\"}}\n\n",
        ));
        let missing_envelope_type = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"error\":{\"type\":\"rate_limit_error\"}}\n\n",
        ));
        let contradictory_event_name = parse_event_payload(&Bytes::from_static(
            b"event: ping\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}\n\n",
        ));
        assert_eq!(anthropic(&exact), Some(StreamRetryReason::Overloaded));
        assert_eq!(
            anthropic(&event_only_overload),
            Some(StreamRetryReason::Overloaded)
        );
        assert_eq!(anthropic(&unknown), None);
        assert_eq!(
            anthropic(&rate_limited),
            Some(StreamRetryReason::RateLimited)
        );
        assert_eq!(anthropic(&prose_only), None);
        assert_eq!(anthropic(&missing_envelope_type), None);
        assert_eq!(anthropic(&contradictory_event_name), None);
    }
}
