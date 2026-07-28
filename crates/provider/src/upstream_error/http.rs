use any2api_domain::{RetrySafety, UpstreamErrorClassification, UpstreamErrorKind};
use http::StatusCode;

use super::retry_after::retry_after_hint;
use crate::api::UpstreamResponseMeta;

pub(crate) fn classify_status(
    meta: &UpstreamResponseMeta,
    not_found: UpstreamErrorKind,
) -> UpstreamErrorClassification {
    let kind = if meta.status.is_server_error() {
        UpstreamErrorKind::Transient
    } else {
        match meta.status {
            StatusCode::BAD_REQUEST => UpstreamErrorKind::InvalidRequest,
            StatusCode::UNAUTHORIZED => UpstreamErrorKind::Authentication,
            StatusCode::PAYMENT_REQUIRED => UpstreamErrorKind::QuotaExhausted,
            StatusCode::FORBIDDEN => UpstreamErrorKind::PermissionDenied,
            StatusCode::NOT_FOUND => not_found,
            StatusCode::TOO_MANY_REQUESTS => UpstreamErrorKind::RateLimited,
            StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_EARLY => UpstreamErrorKind::Transient,
            _ => UpstreamErrorKind::Unknown,
        }
    };
    UpstreamErrorClassification::new(kind, retry_safety(kind), retry_after_hint(&meta.headers))
}

pub(crate) const fn refine_kind(
    baseline: UpstreamErrorKind,
    provider: Option<UpstreamErrorKind>,
) -> UpstreamErrorKind {
    let Some(provider) = provider else {
        return baseline;
    };
    match baseline {
        UpstreamErrorKind::InvalidRequest => match provider {
            UpstreamErrorKind::InvalidRequest | UpstreamErrorKind::ModelUnavailable => provider,
            _ => baseline,
        },
        UpstreamErrorKind::Authentication => UpstreamErrorKind::Authentication,
        UpstreamErrorKind::PermissionDenied => match provider {
            UpstreamErrorKind::PermissionDenied | UpstreamErrorKind::QuotaExhausted => provider,
            _ => baseline,
        },
        UpstreamErrorKind::QuotaExhausted => UpstreamErrorKind::QuotaExhausted,
        UpstreamErrorKind::RateLimited => match provider {
            UpstreamErrorKind::RateLimited | UpstreamErrorKind::QuotaExhausted => provider,
            _ => baseline,
        },
        UpstreamErrorKind::ModelUnavailable => UpstreamErrorKind::ModelUnavailable,
        UpstreamErrorKind::OperationUnavailable => UpstreamErrorKind::OperationUnavailable,
        UpstreamErrorKind::Transient => UpstreamErrorKind::Transient,
        UpstreamErrorKind::Unknown => provider,
    }
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

#[cfg(test)]
mod tests {
    use super::{classify_status, refine_kind};
    use any2api_domain::UpstreamErrorKind;
    use http::{HeaderMap, StatusCode};

    use crate::api::UpstreamResponseMeta;

    #[test]
    fn provider_body_can_only_refine_a_compatible_http_status() {
        assert_eq!(
            refine_kind(
                UpstreamErrorKind::Authentication,
                Some(UpstreamErrorKind::RateLimited),
            ),
            UpstreamErrorKind::Authentication
        );
        assert_eq!(
            refine_kind(
                UpstreamErrorKind::Transient,
                Some(UpstreamErrorKind::Authentication),
            ),
            UpstreamErrorKind::Transient
        );
        assert_eq!(
            refine_kind(
                UpstreamErrorKind::RateLimited,
                Some(UpstreamErrorKind::QuotaExhausted),
            ),
            UpstreamErrorKind::QuotaExhausted
        );
    }

    #[test]
    fn every_server_error_status_has_a_transient_baseline() {
        for status in [500, 501, 529, 599] {
            let classified = classify_status(
                &UpstreamResponseMeta {
                    status: StatusCode::from_u16(status).expect("server error status"),
                    headers: HeaderMap::new(),
                },
                UpstreamErrorKind::Unknown,
            );
            assert_eq!(classified.kind(), UpstreamErrorKind::Transient);
        }
    }
}
