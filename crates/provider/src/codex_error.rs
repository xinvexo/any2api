use any2api_domain::{RetrySafety, UpstreamErrorClassification, UpstreamErrorKind};
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
    kind: Option<String>,
    code: Option<String>,
}

pub(crate) fn classify(
    meta: &UpstreamResponseMeta,
    bounded_body: &[u8],
) -> UpstreamErrorClassification {
    let parsed = serde_json::from_slice::<ErrorEnvelope>(bounded_body).ok();
    let provider_kind = parsed.as_ref().and_then(|envelope| {
        classify_code(
            envelope.error.code.as_deref(),
            envelope.error.kind.as_deref(),
        )
    });
    let kind =
        provider_kind.unwrap_or_else(|| classify_status(meta, UpstreamErrorKind::Unknown).kind());
    let safety = match kind {
        UpstreamErrorKind::Authentication
        | UpstreamErrorKind::PermissionDenied
        | UpstreamErrorKind::QuotaExhausted
        | UpstreamErrorKind::RateLimited
        | UpstreamErrorKind::ModelUnavailable
        | UpstreamErrorKind::OperationUnavailable => RetrySafety::RejectedBeforeExecution,
        _ => classify_status(meta, UpstreamErrorKind::Unknown).retry_safety(),
    };
    UpstreamErrorClassification::new(kind, safety, retry_after_hint(&meta.headers))
}

fn classify_code(code: Option<&str>, kind: Option<&str>) -> Option<UpstreamErrorKind> {
    [code, kind].into_iter().flatten().find_map(|value| {
        let normalized = value.to_ascii_lowercase();
        match normalized.as_str() {
            "invalid_api_key" | "authentication_error" => Some(UpstreamErrorKind::Authentication),
            "insufficient_quota" | "quota_exceeded" | "billing_hard_limit_reached" => {
                Some(UpstreamErrorKind::QuotaExhausted)
            }
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
    use any2api_domain::{RetrySafety, UpstreamErrorKind};
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
        assert_eq!(quota.kind(), UpstreamErrorKind::QuotaExhausted);
        assert_eq!(quota.retry_safety(), RetrySafety::RejectedBeforeExecution);

        let model = classify(
            &UpstreamResponseMeta {
                status: StatusCode::NOT_FOUND,
                headers: HeaderMap::new(),
            },
            br#"{"error":{"code":"model_not_found"}}"#,
        );
        assert_eq!(model.kind(), UpstreamErrorKind::ModelUnavailable);

        let transient = classify(
            &UpstreamResponseMeta {
                status: StatusCode::SERVICE_UNAVAILABLE,
                headers: HeaderMap::new(),
            },
            b"{}",
        );
        assert_eq!(transient.kind(), UpstreamErrorKind::Transient);
        assert_eq!(transient.retry_safety(), RetrySafety::Ambiguous);
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
            br#"{"error":{"type":"rate_limit_error"}}"#,
        );
        assert_eq!(classified.kind(), UpstreamErrorKind::RateLimited);
        assert!(classified.retry_after().is_some());
    }
}
