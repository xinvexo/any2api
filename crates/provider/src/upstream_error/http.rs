use any2api_domain::{
    RetrySafety, UpstreamErrorClassification, UpstreamErrorKind, UpstreamFailureAttribution,
};
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
            StatusCode::BAD_REQUEST
            | StatusCode::CONFLICT
            | StatusCode::PAYLOAD_TOO_LARGE
            | StatusCode::UNPROCESSABLE_ENTITY => UpstreamErrorKind::InvalidRequest,
            StatusCode::UNAUTHORIZED => UpstreamErrorKind::Authentication,
            StatusCode::PAYMENT_REQUIRED => UpstreamErrorKind::QuotaExhausted,
            StatusCode::FORBIDDEN => UpstreamErrorKind::PermissionDenied,
            StatusCode::NOT_FOUND => not_found,
            StatusCode::TOO_MANY_REQUESTS => UpstreamErrorKind::RateLimited,
            StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_EARLY => UpstreamErrorKind::Transient,
            _ => UpstreamErrorKind::Unknown,
        }
    };
    let retry_safety = match meta.status {
        StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_EARLY => RetrySafety::RejectedBeforeExecution,
        _ => kind.default_retry_safety(),
    };
    UpstreamErrorClassification::new(kind, retry_safety, retry_after_hint(&meta.headers))
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
            UpstreamErrorKind::InvalidRequest
            | UpstreamErrorKind::QuotaExhausted
            | UpstreamErrorKind::ModelUnavailable => provider,
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
        UpstreamErrorKind::OperationUnavailable => match provider {
            UpstreamErrorKind::ModelUnavailable | UpstreamErrorKind::OperationUnavailable => {
                provider
            }
            _ => baseline,
        },
        UpstreamErrorKind::Transient => UpstreamErrorKind::Transient,
        UpstreamErrorKind::Unknown => provider,
    }
}

pub(crate) const fn retry_safety_after_refinement(
    baseline: UpstreamErrorClassification,
    refined_kind: UpstreamErrorKind,
) -> RetrySafety {
    match refined_kind.default_retry_safety() {
        RetrySafety::Ambiguous => baseline.retry_safety(),
        safety => safety,
    }
}

pub(crate) fn declared_attribution(
    declared: Option<UpstreamErrorKind>,
    refined: UpstreamErrorKind,
) -> UpstreamFailureAttribution {
    if !matches!(declared, Some(kind) if kind == refined) {
        return UpstreamFailureAttribution::Unattributed;
    }
    match refined {
        UpstreamErrorKind::Authentication => UpstreamFailureAttribution::Authentication,
        UpstreamErrorKind::PermissionDenied | UpstreamErrorKind::QuotaExhausted => {
            UpstreamFailureAttribution::Credential
        }
        UpstreamErrorKind::RateLimited | UpstreamErrorKind::ModelUnavailable => {
            UpstreamFailureAttribution::CredentialModel
        }
        UpstreamErrorKind::OperationUnavailable => UpstreamFailureAttribution::RouteOperation,
        UpstreamErrorKind::InvalidRequest
        | UpstreamErrorKind::Transient
        | UpstreamErrorKind::Unknown => UpstreamFailureAttribution::Unattributed,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_status, declared_attribution, refine_kind};
    use any2api_domain::{RetrySafety, UpstreamErrorKind, UpstreamFailureAttribution};
    use http::{HeaderMap, StatusCode};

    use crate::api::UpstreamResponseMeta;

    #[test]
    fn provider_body_can_only_refine_a_compatible_http_status() {
        assert_eq!(
            refine_kind(
                UpstreamErrorKind::InvalidRequest,
                Some(UpstreamErrorKind::QuotaExhausted),
            ),
            UpstreamErrorKind::QuotaExhausted
        );
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
    fn only_a_matching_declared_kind_expands_failure_attribution() {
        assert_eq!(
            declared_attribution(
                Some(UpstreamErrorKind::Authentication),
                UpstreamErrorKind::Authentication,
            ),
            UpstreamFailureAttribution::Authentication
        );
        assert_eq!(
            declared_attribution(
                Some(UpstreamErrorKind::RateLimited),
                UpstreamErrorKind::Authentication,
            ),
            UpstreamFailureAttribution::Unattributed
        );
        assert_eq!(
            declared_attribution(None, UpstreamErrorKind::PermissionDenied),
            UpstreamFailureAttribution::Unattributed
        );
    }

    #[test]
    fn every_server_error_status_has_an_ambiguous_transient_baseline() {
        for status in [500, 501, 529, 599] {
            let classified = classify_status(
                &UpstreamResponseMeta {
                    status: StatusCode::from_u16(status).expect("server error status"),
                    headers: HeaderMap::new(),
                },
                UpstreamErrorKind::Unknown,
            );
            assert_eq!(classified.kind(), UpstreamErrorKind::Transient);
            assert_eq!(classified.retry_safety(), RetrySafety::Ambiguous);
        }
    }

    #[test]
    fn standard_pre_execution_rejections_are_safe_transient_failures() {
        for status in [StatusCode::REQUEST_TIMEOUT, StatusCode::TOO_EARLY] {
            let classified = classify_status(
                &UpstreamResponseMeta {
                    status,
                    headers: HeaderMap::new(),
                },
                UpstreamErrorKind::Unknown,
            );
            assert_eq!(classified.kind(), UpstreamErrorKind::Transient);
            assert_eq!(
                classified.retry_safety(),
                RetrySafety::RejectedBeforeExecution
            );
        }
    }

    #[test]
    fn misdirected_request_stays_ambiguous_without_new_connection_control() {
        let classified = classify_status(
            &UpstreamResponseMeta {
                status: StatusCode::MISDIRECTED_REQUEST,
                headers: HeaderMap::new(),
            },
            UpstreamErrorKind::Unknown,
        );

        assert_eq!(classified.kind(), UpstreamErrorKind::Unknown);
        assert_eq!(classified.retry_safety(), RetrySafety::Ambiguous);
    }

    #[test]
    fn conflict_large_and_unprocessable_requests_are_client_rejections() {
        for status in [
            StatusCode::CONFLICT,
            StatusCode::PAYLOAD_TOO_LARGE,
            StatusCode::UNPROCESSABLE_ENTITY,
        ] {
            let classified = classify_status(
                &UpstreamResponseMeta {
                    status,
                    headers: HeaderMap::new(),
                },
                UpstreamErrorKind::Unknown,
            );

            assert_eq!(
                classified.kind(),
                UpstreamErrorKind::InvalidRequest,
                "status {status}"
            );
            assert_eq!(
                classified.retry_safety(),
                RetrySafety::Ambiguous,
                "status {status}"
            );
        }
    }
}
