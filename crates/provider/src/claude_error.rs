use any2api_domain::{
    ProtocolOperation, RetrySafety, UpstreamErrorClassification, UpstreamErrorKind,
};
use http::StatusCode;
use serde::Deserialize;

use crate::{
    api::UpstreamResponseMeta, http_error::classify_status, retry_after::retry_after_hint,
};

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetails,
}

#[derive(Deserialize)]
struct ErrorDetails {
    #[serde(rename = "type")]
    kind: String,
}

pub(crate) fn classify(
    operation: ProtocolOperation,
    meta: &UpstreamResponseMeta,
    bounded_body: &[u8],
) -> UpstreamErrorClassification {
    if operation == ProtocolOperation::MessagesCountTokens && meta.status == StatusCode::NOT_FOUND {
        return UpstreamErrorClassification::new(
            UpstreamErrorKind::OperationUnavailable,
            RetrySafety::RejectedBeforeExecution,
            retry_after_hint(&meta.headers),
        );
    }
    let body_kind = serde_json::from_slice::<ErrorEnvelope>(bounded_body)
        .ok()
        .and_then(|envelope| classify_type(&envelope.error.kind));
    let not_found = if operation == ProtocolOperation::Messages {
        UpstreamErrorKind::ModelUnavailable
    } else {
        UpstreamErrorKind::Unknown
    };
    let fallback = classify_status(meta, not_found);
    let kind = body_kind.unwrap_or_else(|| fallback.kind());
    let safety = match kind {
        UpstreamErrorKind::Authentication
        | UpstreamErrorKind::PermissionDenied
        | UpstreamErrorKind::QuotaExhausted
        | UpstreamErrorKind::RateLimited
        | UpstreamErrorKind::ModelUnavailable
        | UpstreamErrorKind::OperationUnavailable => RetrySafety::RejectedBeforeExecution,
        _ => fallback.retry_safety(),
    };
    UpstreamErrorClassification::new(kind, safety, retry_after_hint(&meta.headers))
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
