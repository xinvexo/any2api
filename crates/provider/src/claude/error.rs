//! Claude-specific upstream error classification.

use any2api_domain::{
    ProtocolOperation, RetrySafety, UpstreamError, UpstreamErrorClassification, UpstreamErrorKind,
};
use http::StatusCode;
use serde::Deserialize;

use crate::{
    api::UpstreamResponseMeta,
    upstream_error::{
        http::{classify_status, refine_kind},
        retry_after::retry_after_hint,
    },
};

#[derive(Deserialize)]
struct ErrorEnvelope {
    #[serde(rename = "type")]
    envelope_type: String,
    error: ErrorDetails,
}

#[derive(Deserialize)]
struct ErrorDetails {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<String>,
}

pub(crate) fn classify(
    operation: ProtocolOperation,
    meta: &UpstreamResponseMeta,
    bounded_body: &[u8],
) -> UpstreamError {
    let parsed = serde_json::from_slice::<ErrorEnvelope>(bounded_body)
        .ok()
        .filter(|envelope| envelope.envelope_type == "error");
    let message = parsed
        .as_ref()
        .and_then(|envelope| envelope.error.message.clone());
    if operation == ProtocolOperation::MessagesCountTokens && meta.status == StatusCode::NOT_FOUND {
        return UpstreamError::new(
            UpstreamErrorClassification::new(
                UpstreamErrorKind::OperationUnavailable,
                RetrySafety::RejectedBeforeExecution,
                retry_after_hint(&meta.headers),
            ),
            message,
        );
    }
    let body_kind = parsed
        .as_ref()
        .and_then(|envelope| envelope.error.kind.as_deref())
        .and_then(classify_type);
    let not_found = if operation == ProtocolOperation::Messages {
        UpstreamErrorKind::ModelUnavailable
    } else {
        UpstreamErrorKind::Unknown
    };
    let fallback = classify_status(meta, not_found);
    let kind = refine_kind(fallback.kind(), body_kind);
    let safety = match kind {
        UpstreamErrorKind::Authentication
        | UpstreamErrorKind::PermissionDenied
        | UpstreamErrorKind::QuotaExhausted
        | UpstreamErrorKind::RateLimited
        | UpstreamErrorKind::ModelUnavailable
        | UpstreamErrorKind::OperationUnavailable => RetrySafety::RejectedBeforeExecution,
        _ => fallback.retry_safety(),
    };
    UpstreamError::new(
        UpstreamErrorClassification::new(kind, safety, retry_after_hint(&meta.headers)),
        message,
    )
}

fn classify_type(value: &str) -> Option<UpstreamErrorKind> {
    match value {
        "authentication_error" => Some(UpstreamErrorKind::Authentication),
        "permission_error" => Some(UpstreamErrorKind::PermissionDenied),
        "billing_error" => Some(UpstreamErrorKind::QuotaExhausted),
        "rate_limit_error" => Some(UpstreamErrorKind::RateLimited),
        "not_found_error" => Some(UpstreamErrorKind::ModelUnavailable),
        "invalid_request_error" => Some(UpstreamErrorKind::InvalidRequest),
        "overloaded_error" | "api_error" => Some(UpstreamErrorKind::Transient),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolOperation, UpstreamErrorKind};
    use http::{HeaderMap, StatusCode};

    use super::classify;
    use crate::api::UpstreamResponseMeta;

    #[test]
    fn preserves_anthropic_error_message_independently_of_classification() {
        let error = classify(
            ProtocolOperation::Messages,
            &UpstreamResponseMeta {
                status: StatusCode::BAD_REQUEST,
                headers: HeaderMap::new(),
            },
            br#"{"type":"error","error":{"type":"invalid_request_error","message":"Official Anthropic detail"}}"#,
        );

        assert_eq!(
            error.classification().kind(),
            UpstreamErrorKind::InvalidRequest
        );
        assert_eq!(error.official_message(), Some("Official Anthropic detail"));
    }

    #[test]
    fn ignores_messages_outside_the_declared_anthropic_error_envelope() {
        for body in [
            br#"{"error":{"type":"invalid_request_error","message":"not official"}}"#.as_slice(),
            br#"{"type":"response","error":{"type":"invalid_request_error","message":"not official"}}"#.as_slice(),
        ] {
            let error = classify(
                ProtocolOperation::Messages,
                &UpstreamResponseMeta {
                    status: StatusCode::BAD_REQUEST,
                    headers: HeaderMap::new(),
                },
                body,
            );

            assert_eq!(error.official_message(), None);
            assert_eq!(
                error.classification().kind(),
                UpstreamErrorKind::InvalidRequest
            );
        }
    }
}
