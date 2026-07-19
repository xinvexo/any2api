use any2api_domain::{RetrySafety, UpstreamErrorClassification, UpstreamErrorKind};
use http::StatusCode;

use crate::{api::UpstreamResponseMeta, retry_after::retry_after_hint};

pub(crate) fn classify_status(
    meta: &UpstreamResponseMeta,
    not_found: UpstreamErrorKind,
) -> UpstreamErrorClassification {
    let kind = match meta.status {
        StatusCode::BAD_REQUEST => UpstreamErrorKind::InvalidRequest,
        StatusCode::UNAUTHORIZED => UpstreamErrorKind::Authentication,
        StatusCode::PAYMENT_REQUIRED => UpstreamErrorKind::QuotaExhausted,
        StatusCode::FORBIDDEN => UpstreamErrorKind::PermissionDenied,
        StatusCode::NOT_FOUND => not_found,
        StatusCode::TOO_MANY_REQUESTS => UpstreamErrorKind::RateLimited,
        StatusCode::REQUEST_TIMEOUT
        | StatusCode::TOO_EARLY
        | StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE
        | StatusCode::GATEWAY_TIMEOUT => UpstreamErrorKind::Transient,
        _ => UpstreamErrorKind::Unknown,
    };
    UpstreamErrorClassification::new(kind, retry_safety(kind), retry_after_hint(&meta.headers))
}

const fn retry_safety(kind: UpstreamErrorKind) -> RetrySafety {
    match kind {
        UpstreamErrorKind::Authentication
        | UpstreamErrorKind::PermissionDenied
        | UpstreamErrorKind::QuotaExhausted
        | UpstreamErrorKind::RateLimited
        | UpstreamErrorKind::ModelUnavailable
        | UpstreamErrorKind::OperationUnavailable => RetrySafety::RejectedBeforeExecution,
        UpstreamErrorKind::InvalidRequest
        | UpstreamErrorKind::Transient
        | UpstreamErrorKind::Unknown => RetrySafety::Ambiguous,
    }
}
