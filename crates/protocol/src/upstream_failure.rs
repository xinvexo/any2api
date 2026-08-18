use any2api_domain::RetrySafety;
use bytes::Bytes;
use serde_json::value::RawValue;

use crate::{
    api::{
        ProtocolRetryDelayBasis, ProtocolUpstreamFailureEvidence, SseEventPayload,
        StreamTermination,
    },
    raw_json::{json_string, object_field_raw, top_fields},
    telemetry::raw_event_type,
};

pub(crate) fn openai_stream(
    payload: &SseEventPayload,
    termination: StreamTermination,
) -> Option<ProtocolUpstreamFailureEvidence> {
    let SseEventPayload::Json(data) = payload else {
        return None;
    };
    if termination != StreamTermination::Failed {
        return None;
    }
    let evidence = ProtocolUpstreamFailureEvidence::new(data.data().clone());
    if is_openai_overload(data.data()) {
        Some(
            evidence
                .with_retry_safety_override(RetrySafety::RejectedBeforeExecution)
                .with_retry_delay_basis(ProtocolRetryDelayBasis::RequestAttempts),
        )
    } else {
        Some(evidence)
    }
}

pub(crate) fn anthropic_stream(
    payload: &SseEventPayload,
    termination: StreamTermination,
) -> Option<ProtocolUpstreamFailureEvidence> {
    let SseEventPayload::Json(data) = payload else {
        return None;
    };
    if termination != StreamTermination::Failed {
        return None;
    }
    let evidence = ProtocolUpstreamFailureEvidence::new(data.data().clone());
    let [kind, error] = top_fields(data.data(), ["type", "error"]);
    let event_kind = raw_event_type(data.event_name(), kind);
    let nested_kind = error
        .filter(|error| is_object(error))
        .and_then(|error| object_field_raw(error.get().as_bytes(), "type"))
        .and_then(json_string);
    match nested_kind.as_deref() {
        Some("overloaded_error") if event_kind.as_deref() == Some("error") => Some(
            evidence
                .with_retry_safety_override(RetrySafety::RejectedBeforeExecution)
                .with_retry_delay_basis(ProtocolRetryDelayBasis::RequestAttempts),
        ),
        Some("rate_limit_error")
            if kind.and_then(json_string).as_deref() == Some("error")
                && data.event_name().is_none_or(|name| name == "error") =>
        {
            Some(evidence.with_retry_safety_override(RetrySafety::RejectedBeforeExecution))
        }
        _ => Some(evidence),
    }
}

pub(crate) fn responses_buffered(body: &Bytes) -> Option<ProtocolUpstreamFailureEvidence> {
    let [status, error] = top_fields(body, ["status", "error"]);
    (status.and_then(json_string).as_deref() == Some("failed") && error.is_some_and(is_object))
        .then(|| ProtocolUpstreamFailureEvidence::new(body.clone()))
}

pub(crate) fn openai_error_buffered(body: &Bytes) -> Option<ProtocolUpstreamFailureEvidence> {
    let [error] = top_fields(body, ["error"]);
    error
        .is_some_and(is_object)
        .then(|| ProtocolUpstreamFailureEvidence::new(body.clone()))
}

pub(crate) fn anthropic_buffered(body: &Bytes) -> Option<ProtocolUpstreamFailureEvidence> {
    let [kind, error] = top_fields(body, ["type", "error"]);
    (kind.and_then(json_string).as_deref() == Some("error") && error.is_some_and(is_object))
        .then(|| ProtocolUpstreamFailureEvidence::new(body.clone()))
}

fn is_openai_overload(body: &[u8]) -> bool {
    let [error, response, code] = top_fields(body, ["error", "response", "code"]);
    let nested_code = error
        .filter(|error| is_object(error))
        .and_then(|error| object_field_raw(error.get().as_bytes(), "code"))
        .and_then(json_string);
    let response_code = response
        .filter(|response| is_object(response))
        .and_then(|response| object_field_raw(response.get().as_bytes(), "error"))
        .filter(|error| is_object(error))
        .and_then(|error| object_field_raw(error.get().as_bytes(), "code"))
        .and_then(json_string);
    [nested_code, response_code, code.and_then(json_string)]
        .into_iter()
        .flatten()
        .any(|code| code == "server_is_overloaded")
}

fn is_object(value: &RawValue) -> bool {
    value.get().trim_ascii_start().starts_with('{')
}

#[cfg(test)]
mod tests {
    use any2api_domain::RetrySafety;

    use super::*;
    use crate::sse::parse_event_payload;

    #[test]
    fn every_explicit_openai_failure_has_evidence_but_only_exact_overload_overrides_safety() {
        let exact = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"code\":\"server_is_overloaded\",\"message\":\"busy\"}}\n\n",
        ))
        .expect("exact payload");
        let generic = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"code\":\"server_error\",\"message\":\"busy\"}}\n\n",
        ))
        .expect("generic payload");
        let overload_in_second_declared_location = parse_event_payload(&Bytes::from_static(
            b"event: response.failed\ndata: {\"type\":\"response.failed\",\"error\":{\"code\":\"server_error\"},\"response\":{\"error\":{\"code\":\"server_is_overloaded\"}}}\n\n",
        ))
        .expect("nested overload payload");
        let exact = openai_stream(&exact, StreamTermination::Failed).expect("exact evidence");
        assert_eq!(
            exact.retry_safety_override(),
            Some(RetrySafety::RejectedBeforeExecution)
        );
        assert_eq!(
            exact.retry_delay_basis(),
            ProtocolRetryDelayBasis::RequestAttempts
        );
        let generic = openai_stream(&generic, StreamTermination::Failed).expect("generic evidence");
        assert_eq!(generic.retry_safety_override(), None);
        assert_eq!(
            generic.retry_delay_basis(),
            ProtocolRetryDelayBasis::CandidateAttempts
        );
        assert_eq!(
            openai_stream(
                &overload_in_second_declared_location,
                StreamTermination::Failed
            )
            .expect("nested response overload evidence")
            .retry_safety_override(),
            Some(RetrySafety::RejectedBeforeExecution)
        );
        assert!(!format!("{exact:?}").contains("server_is_overloaded"));
    }

    #[test]
    fn anthropic_safety_override_requires_declared_structured_types() {
        let overloaded = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\"}}\n\n",
        ))
        .expect("overload payload");
        let rate_limited = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"rate_limit_error\"}}\n\n",
        ))
        .expect("rate limit payload");
        let prose = parse_event_payload(&Bytes::from_static(
            b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"api_error\",\"message\":\"rate_limit_error\"}}\n\n",
        ))
        .expect("prose payload");
        let overloaded =
            anthropic_stream(&overloaded, StreamTermination::Failed).expect("overload evidence");
        assert_eq!(
            overloaded.retry_safety_override(),
            Some(RetrySafety::RejectedBeforeExecution)
        );
        assert_eq!(
            overloaded.retry_delay_basis(),
            ProtocolRetryDelayBasis::RequestAttempts
        );
        let rate_limited =
            anthropic_stream(&rate_limited, StreamTermination::Failed).expect("rate evidence");
        assert_eq!(
            rate_limited.retry_safety_override(),
            Some(RetrySafety::RejectedBeforeExecution)
        );
        assert_eq!(
            rate_limited.retry_delay_basis(),
            ProtocolRetryDelayBasis::CandidateAttempts
        );
        assert_eq!(
            anthropic_stream(&prose, StreamTermination::Failed)
                .expect("generic evidence")
                .retry_safety_override(),
            None
        );
    }

    #[test]
    fn buffered_matchers_require_exact_top_level_shapes() {
        assert!(
            responses_buffered(&Bytes::from_static(
                br#"{"status":"failed","error":{"code":"server_error"}}"#
            ))
            .is_some()
        );
        assert!(
            responses_buffered(&Bytes::from_static(
                br#"{"status":"incomplete","error":{"code":"server_error"}}"#
            ))
            .is_none()
        );
        assert!(
            openai_error_buffered(&Bytes::from_static(
                br#"{"result":{"error":{"code":"server_error"}}}"#
            ))
            .is_none()
        );
        assert!(
            anthropic_buffered(&Bytes::from_static(
                br#"{"type":"error","error":{"type":"api_error"}}"#
            ))
            .is_some()
        );
    }
}
