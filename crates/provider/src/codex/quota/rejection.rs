//! Codex quota rejection evidence and account-free egress probe results.

use http::StatusCode;
use serde_json::Value;

use crate::api::{OAuthProviderEgressStatus, OAuthQuotaRejection, UpstreamResponseMeta};

const PROVIDER_EGRESS_CODE: &str = "unsupported_country_region_territory";
const ACCOUNT_RESTRICTION_CODES: [&str; 3] = [
    "account_deactivated",
    "account_suspended",
    "account_disabled",
];

pub(crate) fn classify_quota_rejection(
    meta: &UpstreamResponseMeta,
    bounded_body: &[u8],
) -> OAuthQuotaRejection {
    if meta.status != StatusCode::FORBIDDEN {
        return OAuthQuotaRejection::Unclassified;
    }
    classify_declared_code(bounded_body).unwrap_or(OAuthQuotaRejection::Unclassified)
}

pub(crate) fn classify_egress(
    meta: &UpstreamResponseMeta,
    _bounded_body: &[u8],
) -> OAuthProviderEgressStatus {
    if meta.status.is_success() || meta.status == StatusCode::UNAUTHORIZED {
        OAuthProviderEgressStatus::Reachable
    } else if meta.status == StatusCode::FORBIDDEN {
        OAuthProviderEgressStatus::Restricted
    } else {
        OAuthProviderEgressStatus::Unverified
    }
}

fn classify_declared_code(body: &[u8]) -> Option<OAuthQuotaRejection> {
    let parsed = serde_json::from_slice::<Value>(body).ok()?;
    let object = parsed.as_object()?;
    let top_level = object.get("code").and_then(Value::as_str);
    let error = object.get("error").and_then(Value::as_object);
    let error_code = error
        .and_then(|object| object.get("code"))
        .and_then(Value::as_str);
    let error_type = error
        .and_then(|object| object.get("type"))
        .and_then(Value::as_str);
    let mut classified = None;
    for code in [top_level, error_code, error_type].into_iter().flatten() {
        let Some(next) = classify_code(code) else {
            continue;
        };
        match classified {
            None => classified = Some(next),
            Some(current) if current == next => {}
            Some(_) => return None,
        }
    }
    classified
}

fn classify_code(value: &str) -> Option<OAuthQuotaRejection> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == PROVIDER_EGRESS_CODE {
        Some(OAuthQuotaRejection::ProviderEgressRestricted)
    } else if ACCOUNT_RESTRICTION_CODES.contains(&normalized.as_str()) {
        Some(OAuthQuotaRejection::AccountRestricted)
    } else {
        None
    }
}
