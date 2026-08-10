use any2api_domain::{UpstreamError, UpstreamErrorClassification, UpstreamErrorKind};
use serde::Deserialize;

use crate::{
    api::UpstreamResponseMeta,
    upstream_error::{
        http::{classify_status, declared_attribution, refine_kind, retry_safety_after_refinement},
        retry_after::retry_after_hint,
    },
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
    let kind = if baseline.kind() == UpstreamErrorKind::RateLimited
        && provider_kind == Some(UpstreamErrorKind::Transient)
    {
        // Moonshot explicitly uses HTTP 429 for engine capacity overload. It is
        // a service failure, not a credential-scoped rate limit.
        UpstreamErrorKind::Transient
    } else {
        refine_kind(baseline.kind(), provider_kind)
    };
    let classification = UpstreamErrorClassification::new(
        kind,
        retry_safety_after_refinement(baseline, kind),
        retry_after_hint(&meta.headers),
    )
    .with_attribution(declared_attribution(provider_kind, kind));
    let message = parsed.and_then(|envelope| envelope.error.message);
    UpstreamError::new(classification, message)
}

fn classify_code(code: Option<&str>, kind: Option<&str>) -> Option<UpstreamErrorKind> {
    [code, kind]
        .into_iter()
        .flatten()
        .find_map(|value| match value.to_ascii_lowercase().as_str() {
            "invalid_authentication_error" | "incorrect_api_key_error" => {
                Some(UpstreamErrorKind::Authentication)
            }
            "permission_denied_error" => Some(UpstreamErrorKind::PermissionDenied),
            "exceeded_current_quota_error" => Some(UpstreamErrorKind::QuotaExhausted),
            "rate_limit_reached_error" => Some(UpstreamErrorKind::RateLimited),
            "resource_not_found_error" => Some(UpstreamErrorKind::ModelUnavailable),
            "content_filter" | "invalid_request_error" => Some(UpstreamErrorKind::InvalidRequest),
            "engine_overloaded_error"
            | "client_closed_request"
            | "server_error"
            | "unexpected_output"
            | "server_unavailable" => Some(UpstreamErrorKind::Transient),
            _ => None,
        })
}
