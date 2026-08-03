//! Grok Build error codes that carry account-scoped meaning.

use any2api_domain::{
    UpstreamError, UpstreamErrorClassification, UpstreamErrorKind, UpstreamQuotaExhaustion,
};
use http::StatusCode;
use serde::Deserialize;

use crate::{
    api::{OAuthQuotaRejection, UpstreamResponseMeta},
    upstream_error,
};

const FREE_USAGE_EXHAUSTED: &str = "subscription:free-usage-exhausted";
const BLOCKED_USER: &str = "unauthorized:blocked-user";
const ACTUAL_LIMIT_MARKER: &str = "tokens (actual/limit):";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
struct GrokErrorEnvelope {
    code: Option<String>,
    error: Option<GrokErrorField>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum GrokErrorField {
    Message(String),
    Details {
        code: Option<String>,
        message: Option<String>,
    },
}

pub(crate) fn classify(meta: &UpstreamResponseMeta, bounded_body: &[u8]) -> UpstreamError {
    let parsed = serde_json::from_slice::<GrokErrorEnvelope>(bounded_body).ok();
    let fallback = upstream_error::openai::classify(meta, bounded_body);
    let message = parsed
        .as_ref()
        .and_then(GrokErrorEnvelope::message)
        .or_else(|| fallback.official_message().map(str::to_owned));
    if matches!(
        meta.status,
        StatusCode::PAYMENT_REQUIRED | StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS
    ) && parsed
        .as_ref()
        .is_some_and(|value| value.has_code(FREE_USAGE_EXHAUSTED))
    {
        let classified = UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            UpstreamErrorKind::QuotaExhausted.default_retry_safety(),
            upstream_error::retry_after::retry_after_hint(&meta.headers),
        );
        let classified = message
            .as_deref()
            .and_then(parse_actual_limit)
            .map_or(classified, |value| classified.with_quota_exhaustion(value));
        return UpstreamError::new(classified, message);
    }
    if meta.status == StatusCode::FORBIDDEN
        && parsed
            .as_ref()
            .is_some_and(|value| value.has_code(BLOCKED_USER))
    {
        return UpstreamError::new(
            UpstreamErrorClassification::new(
                UpstreamErrorKind::PermissionDenied,
                UpstreamErrorKind::PermissionDenied.default_retry_safety(),
                upstream_error::retry_after::retry_after_hint(&meta.headers),
            ),
            message,
        );
    }
    UpstreamError::new(fallback.classification(), message)
}

pub(crate) fn classify_oauth_quota_rejection(
    meta: &UpstreamResponseMeta,
    bounded_body: &[u8],
) -> OAuthQuotaRejection {
    let blocked = meta.status == StatusCode::FORBIDDEN
        && serde_json::from_slice::<GrokErrorEnvelope>(bounded_body)
            .ok()
            .is_some_and(|value| value.has_code(BLOCKED_USER));
    if blocked {
        OAuthQuotaRejection::AccountRestricted
    } else {
        OAuthQuotaRejection::Unclassified
    }
}

impl GrokErrorEnvelope {
    fn has_code(&self, expected: &str) -> bool {
        self.code.as_deref() == Some(expected)
            || match &self.error {
                Some(GrokErrorField::Message(message)) => {
                    message.contains(&format!("WKE={expected}"))
                }
                Some(GrokErrorField::Details { code, .. }) => code.as_deref() == Some(expected),
                None => false,
            }
    }

    fn message(&self) -> Option<String> {
        match &self.error {
            Some(GrokErrorField::Message(message)) => Some(message.clone()),
            Some(GrokErrorField::Details { message, .. }) => message.clone(),
            None => None,
        }
    }
}

fn parse_actual_limit(message: &str) -> Option<UpstreamQuotaExhaustion> {
    let values = message.split_once(ACTUAL_LIMIT_MARKER)?.1.trim_start();
    let (used, remainder) = decimal_prefix(values)?;
    let remainder = remainder.strip_prefix('/')?;
    let (limit, _) = decimal_prefix(remainder)?;
    Some(UpstreamQuotaExhaustion::new(used, limit))
}

fn decimal_prefix(value: &str) -> Option<(u64, &str)> {
    let digits = value
        .char_indices()
        .take_while(|(_, character)| character.is_ascii_digit())
        .last()
        .map_or(0, |(index, character)| index + character.len_utf8());
    if digits == 0 {
        return None;
    }
    let parsed = value[..digits].parse().ok()?;
    (parsed <= MAX_SAFE_INTEGER).then_some((parsed, &value[digits..]))
}

#[cfg(test)]
mod tests {
    use any2api_domain::{UpstreamErrorKind, UpstreamQuotaExhaustion};
    use http::{HeaderMap, StatusCode};

    use super::*;

    fn meta(status: StatusCode) -> UpstreamResponseMeta {
        UpstreamResponseMeta {
            status,
            headers: HeaderMap::new(),
        }
    }

    #[test]
    fn classifies_real_free_exhaustion_and_actual_limit() {
        let classified = classify(
            &meta(StatusCode::TOO_MANY_REQUESTS),
            br#"{"error":{"code":"subscription:free-usage-exhausted","message":"tokens (actual/limit): 1065387/1000000; Usage resets over a rolling 24-hour window"}}"#,
        );

        assert_eq!(
            classified.classification().kind(),
            UpstreamErrorKind::QuotaExhausted
        );
        assert_eq!(
            classified.classification().quota_exhaustion(),
            Some(UpstreamQuotaExhaustion::new(1_065_387, 1_000_000))
        );
        assert_eq!(
            classified.official_message(),
            Some(
                "tokens (actual/limit): 1065387/1000000; Usage resets over a rolling 24-hour window"
            )
        );

        let unsafe_values = classify(
            &meta(StatusCode::TOO_MANY_REQUESTS),
            br#"{"code":"subscription:free-usage-exhausted","error":"tokens (actual/limit): 9007199254740992/9007199254740992"}"#,
        );
        assert_eq!(
            unsafe_values.classification().kind(),
            UpstreamErrorKind::QuotaExhausted
        );
        assert!(unsafe_values.classification().quota_exhaustion().is_none());
        assert_eq!(
            unsafe_values.official_message(),
            Some("tokens (actual/limit): 9007199254740992/9007199254740992")
        );
    }

    #[test]
    fn classifies_blocked_user_without_confusing_generic_forbidden() {
        assert_eq!(
            classify(
                &meta(StatusCode::FORBIDDEN),
                br#"{"code":"unauthorized:blocked-user","error":"User is blocked"}"#,
            )
            .classification()
            .kind(),
            UpstreamErrorKind::PermissionDenied
        );
        let generic = classify(&meta(StatusCode::FORBIDDEN), br#"{"error":"denied"}"#);
        assert_eq!(
            generic.classification().kind(),
            UpstreamErrorKind::PermissionDenied
        );
        assert_eq!(generic.official_message(), Some("denied"));
        assert_eq!(
            classify_oauth_quota_rejection(
                &meta(StatusCode::FORBIDDEN),
                br#"{"code":"unauthorized:blocked-user"}"#,
            ),
            crate::api::OAuthQuotaRejection::AccountRestricted
        );
        assert_eq!(
            classify_oauth_quota_rejection(&meta(StatusCode::FORBIDDEN), br#"{"error":"denied"}"#,),
            crate::api::OAuthQuotaRejection::Unclassified
        );
    }

    #[test]
    fn special_codes_only_apply_in_declared_fields_and_compatible_statuses() {
        let nested = classify(
            &meta(StatusCode::TOO_MANY_REQUESTS),
            br#"{"metadata":{"code":"subscription:free-usage-exhausted"},"error":{"message":"ordinary limit"}}"#,
        );
        assert_eq!(
            nested.classification().kind(),
            UpstreamErrorKind::RateLimited
        );

        let contradictory = classify(
            &meta(StatusCode::UNAUTHORIZED),
            br#"{"error":{"code":"subscription:free-usage-exhausted","message":"not authenticated"}}"#,
        );
        assert_eq!(
            contradictory.classification().kind(),
            UpstreamErrorKind::Authentication
        );
    }
}
