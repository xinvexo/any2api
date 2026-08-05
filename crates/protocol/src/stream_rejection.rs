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
    let kind = raw_event_type(data.event_name(), kind);
    if kind.as_deref() != Some("error") {
        return None;
    }
    error
        .and_then(|error| object_field_raw(error.get().as_bytes(), "type"))
        .and_then(json_string)
        .is_some_and(|kind| kind == "overloaded_error")
        .then_some(StreamRetryReason::Overloaded)
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
    fn recognizes_only_declared_anthropic_overload_types() {
        let exact = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"busy\"}}\n\n",
        ));
        let unknown = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"overloaded\"}}\n\n",
        ));
        assert_eq!(anthropic(&exact), Some(StreamRetryReason::Overloaded));
        assert_eq!(anthropic(&unknown), None);
    }
}
