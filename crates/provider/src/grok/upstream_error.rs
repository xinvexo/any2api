//! Grok Build error codes that carry account-scoped meaning.

use any2api_domain::{
    RetrySafety, UpstreamErrorClassification, UpstreamErrorKind, UpstreamQuotaExhaustion,
};

use crate::{api::UpstreamResponseMeta, upstream_error};

const FREE_USAGE_EXHAUSTED: &str = "subscription:free-usage-exhausted";
const BLOCKED_USER: &str = "unauthorized:blocked-user";
const ACTUAL_LIMIT_MARKER: &str = "tokens (actual/limit):";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(crate) fn classify(
    meta: &UpstreamResponseMeta,
    bounded_body: &[u8],
) -> UpstreamErrorClassification {
    if contains_error_code(bounded_body, FREE_USAGE_EXHAUSTED) {
        let classified = UpstreamErrorClassification::new(
            UpstreamErrorKind::QuotaExhausted,
            RetrySafety::RejectedBeforeExecution,
            upstream_error::retry_after::retry_after_hint(&meta.headers),
        );
        return parse_actual_limit(bounded_body)
            .map_or(classified, |value| classified.with_quota_exhaustion(value));
    }
    if contains_error_code(bounded_body, BLOCKED_USER) {
        return UpstreamErrorClassification::new(
            UpstreamErrorKind::PermissionDenied,
            RetrySafety::RejectedBeforeExecution,
            upstream_error::retry_after::retry_after_hint(&meta.headers),
        );
    }
    upstream_error::openai::classify(meta, bounded_body)
}

fn contains_error_code(body: &[u8], expected: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .is_some_and(|value| value_contains_code(&value, expected))
}

fn value_contains_code(value: &serde_json::Value, expected: &str) -> bool {
    match value {
        serde_json::Value::String(value) => {
            value == expected || value.contains(&format!("WKE={expected}"))
        }
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| value_contains_code(value, expected)),
        serde_json::Value::Object(values) => values
            .values()
            .any(|value| value_contains_code(value, expected)),
        _ => false,
    }
}

fn parse_actual_limit(body: &[u8]) -> Option<UpstreamQuotaExhaustion> {
    let text = std::str::from_utf8(body).ok()?;
    let values = text.split_once(ACTUAL_LIMIT_MARKER)?.1.trim_start();
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

        assert_eq!(classified.kind(), UpstreamErrorKind::QuotaExhausted);
        assert_eq!(
            classified.quota_exhaustion(),
            Some(UpstreamQuotaExhaustion::new(1_065_387, 1_000_000))
        );

        let unsafe_values = classify(
            &meta(StatusCode::TOO_MANY_REQUESTS),
            br#"{"code":"subscription:free-usage-exhausted","error":"tokens (actual/limit): 9007199254740992/9007199254740992"}"#,
        );
        assert_eq!(unsafe_values.kind(), UpstreamErrorKind::QuotaExhausted);
        assert!(unsafe_values.quota_exhaustion().is_none());
    }

    #[test]
    fn classifies_blocked_user_without_confusing_generic_forbidden() {
        assert_eq!(
            classify(
                &meta(StatusCode::FORBIDDEN),
                br#"{"code":"unauthorized:blocked-user","error":"User is blocked"}"#,
            )
            .kind(),
            UpstreamErrorKind::PermissionDenied
        );
        assert_eq!(
            classify(&meta(StatusCode::FORBIDDEN), br#"{"error":"denied"}"#).kind(),
            UpstreamErrorKind::PermissionDenied
        );
    }
}
