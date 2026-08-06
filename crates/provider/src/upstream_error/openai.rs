use any2api_domain::{UpstreamError, UpstreamErrorClassification, UpstreamErrorKind};
use serde::Deserialize;

use super::{
    http::{classify_status, declared_attribution, refine_kind, retry_safety_after_refinement},
    retry_after::retry_after_hint,
};
use crate::api::UpstreamResponseMeta;

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: ErrorDetails,
}

#[derive(Deserialize)]
struct ErrorDetails {
    #[serde(rename = "type")]
    kind: Option<String>,
    code: Option<String>,
    message: Option<String>,
}

pub(crate) fn classify(meta: &UpstreamResponseMeta, bounded_body: &[u8]) -> UpstreamError {
    let parsed = serde_json::from_slice::<ErrorEnvelope>(bounded_body).ok();
    let provider_kind = parsed.as_ref().and_then(|envelope| {
        classify_code(
            envelope.error.code.as_deref(),
            envelope.error.kind.as_deref(),
        )
    });
    let baseline = classify_status(meta, UpstreamErrorKind::OperationUnavailable);
    let kind = refine_kind(baseline.kind(), provider_kind);
    let safety = retry_safety_after_refinement(baseline, kind);
    let classification =
        UpstreamErrorClassification::new(kind, safety, retry_after_hint(&meta.headers))
            .with_attribution(declared_attribution(provider_kind, kind));
    let message = parsed.and_then(|envelope| envelope.error.message);
    UpstreamError::new(classification, message)
}

fn classify_code(code: Option<&str>, kind: Option<&str>) -> Option<UpstreamErrorKind> {
    [code, kind].into_iter().flatten().find_map(|value| {
        let normalized = value.to_ascii_lowercase();
        match normalized.as_str() {
            "invalid_api_key" | "authentication_error" => Some(UpstreamErrorKind::Authentication),
            "credit_balance_exhausted"
            | "organization_spend_limit_exceeded"
            | "project_spend_limit_exceeded"
            | "organization_usage_limit_exceeded"
            | "insufficient_quota"
            | "quota_exceeded"
            | "billing_hard_limit_reached" => Some(UpstreamErrorKind::QuotaExhausted),
            "rate_limit_error" | "rate_limit_exceeded" => Some(UpstreamErrorKind::RateLimited),
            "model_not_found" | "model_not_available" | "unsupported_model" => {
                Some(UpstreamErrorKind::ModelUnavailable)
            }
            "permission_denied" | "permission_error" => Some(UpstreamErrorKind::PermissionDenied),
            "invalid_request_error" => Some(UpstreamErrorKind::InvalidRequest),
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use any2api_domain::{RetrySafety, UpstreamErrorKind, UpstreamFailureAttribution};
    use http::{HeaderMap, HeaderValue, StatusCode, header};

    use super::classify;
    use crate::api::UpstreamResponseMeta;

    #[test]
    fn distinguishes_quota_model_and_ambiguous_server_errors() {
        let quota = classify(
            &UpstreamResponseMeta {
                status: StatusCode::TOO_MANY_REQUESTS,
                headers: HeaderMap::new(),
            },
            br#"{"error":{"type":"insufficient_quota","code":"insufficient_quota"}}"#,
        );
        assert_eq!(
            quota.classification().kind(),
            UpstreamErrorKind::QuotaExhausted
        );
        assert_eq!(
            quota.classification().retry_safety(),
            RetrySafety::RejectedBeforeExecution
        );

        let model = classify(
            &UpstreamResponseMeta {
                status: StatusCode::NOT_FOUND,
                headers: HeaderMap::new(),
            },
            br#"{"error":{"code":"model_not_found"}}"#,
        );
        assert_eq!(
            model.classification().kind(),
            UpstreamErrorKind::ModelUnavailable
        );

        let transient = classify(
            &UpstreamResponseMeta {
                status: StatusCode::SERVICE_UNAVAILABLE,
                headers: HeaderMap::new(),
            },
            b"{}",
        );
        assert_eq!(
            transient.classification().kind(),
            UpstreamErrorKind::Transient
        );
        assert_eq!(
            transient.classification().retry_safety(),
            RetrySafety::Ambiguous
        );
    }

    #[test]
    fn status_only_not_found_is_safe_but_remains_exact_candidate_scoped() {
        let classified = classify(
            &UpstreamResponseMeta {
                status: StatusCode::NOT_FOUND,
                headers: HeaderMap::new(),
            },
            b"{}",
        );

        assert_eq!(
            classified.classification().kind(),
            UpstreamErrorKind::OperationUnavailable
        );
        assert_eq!(
            classified.classification().retry_safety(),
            RetrySafety::RejectedBeforeExecution
        );
        assert_eq!(
            classified.classification().attribution(),
            UpstreamFailureAttribution::Unattributed
        );
    }

    #[test]
    fn recognizes_structured_quota_codes_on_compatible_statuses() {
        for code in [
            "credit_balance_exhausted",
            "organization_spend_limit_exceeded",
            "project_spend_limit_exceeded",
            "organization_usage_limit_exceeded",
            "insufficient_quota",
            "quota_exceeded",
            "billing_hard_limit_reached",
        ] {
            let body = format!(r#"{{"error":{{"code":"{code}"}}}}"#);
            let status = if code == "billing_hard_limit_reached" {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::TOO_MANY_REQUESTS
            };
            let classified = classify(
                &UpstreamResponseMeta {
                    status,
                    headers: HeaderMap::new(),
                },
                body.as_bytes(),
            );

            assert_eq!(
                classified.classification().kind(),
                UpstreamErrorKind::QuotaExhausted,
                "structured quota code {code}"
            );
            assert_eq!(
                classified.classification().retry_safety(),
                RetrySafety::RejectedBeforeExecution,
                "structured quota code {code}"
            );
        }
    }

    #[test]
    fn low_credit_message_without_a_quota_code_stays_invalid_request() {
        let classified = classify(
            &UpstreamResponseMeta {
                status: StatusCode::BAD_REQUEST,
                headers: HeaderMap::new(),
            },
            br#"{"error":{"type":"invalid_request_error","message":"Your credit balance is too low"}}"#,
        );

        assert_eq!(
            classified.classification().kind(),
            UpstreamErrorKind::InvalidRequest
        );
    }

    #[test]
    fn rate_limit_keeps_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RETRY_AFTER, HeaderValue::from_static("9"));
        let classified = classify(
            &UpstreamResponseMeta {
                status: StatusCode::TOO_MANY_REQUESTS,
                headers,
            },
            br#"{"error":{"type":"rate_limit_error","message":"Official retry detail"}}"#,
        );
        assert_eq!(
            classified.classification().kind(),
            UpstreamErrorKind::RateLimited
        );
        assert!(classified.classification().retry_after().is_some());
        assert_eq!(classified.official_message(), Some("Official retry detail"));
    }

    #[test]
    fn body_codes_cannot_contradict_authentication_or_server_status() {
        let authentication = classify(
            &UpstreamResponseMeta {
                status: StatusCode::UNAUTHORIZED,
                headers: HeaderMap::new(),
            },
            br#"{"error":{"type":"rate_limit_error"}}"#,
        );
        assert_eq!(
            authentication.classification().kind(),
            UpstreamErrorKind::Authentication
        );

        let server = classify(
            &UpstreamResponseMeta {
                status: StatusCode::NOT_IMPLEMENTED,
                headers: HeaderMap::new(),
            },
            br#"{"error":{"code":"invalid_api_key"}}"#,
        );
        assert_eq!(server.classification().kind(), UpstreamErrorKind::Transient);
    }
}
