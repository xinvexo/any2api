//! Grok Free token-balance probe contract.

use http::{HeaderValue, Method, header};
use url::Url;

use super::protocol::request_headers;
use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError,
    api::UpstreamResponseMeta,
    grok::upstream_error,
    oauth::quota::{OAuthQuotaTokenBalance, OAuthQuotaTokenBalanceSource, OAuthQuotaUsage},
};

const TOKEN_BALANCE_URL: &str = "https://cli-chat-proxy.grok.com/v1/chat/completions";
const TOKEN_BALANCE_MODEL: &str = "grok-4.5";
const LIMIT_HEADER: &str = "x-ratelimit-limit-tokens";
const REMAINING_HEADER: &str = "x-ratelimit-remaining-tokens";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const PROBE_BODY: &[u8] = br#"{"model":"grok-4.5","stream":false,"max_tokens":1,"messages":[{"role":"user","content":"ping"}]}"#;

pub(crate) fn token_balance_plan(
    token: &OAuthTokenMaterial,
    usage: &OAuthQuotaUsage,
) -> Result<Option<OAuthRequestPlan>, ProviderError> {
    if !is_free(usage) {
        return Ok(None);
    }
    let mut headers = request_headers(token)?;
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "x-grok-model-override",
        HeaderValue::from_static(TOKEN_BALANCE_MODEL),
    );
    Ok(Some(OAuthRequestPlan {
        method: Method::POST,
        url: Url::parse(TOKEN_BALANCE_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers,
        body: PROBE_BODY.to_vec(),
    }))
}

pub(crate) fn parse_token_balance(
    usage: &OAuthQuotaUsage,
    meta: &UpstreamResponseMeta,
    body: &[u8],
) -> Result<Option<OAuthQuotaTokenBalance>, ProviderError> {
    if !is_free(usage) {
        return Ok(None);
    }
    if let Some(exhaustion) = upstream_error::classify(meta, body)
        .classification()
        .quota_exhaustion()
        .filter(|value| value.limit() > 0)
    {
        return Ok(Some(OAuthQuotaTokenBalance {
            source: OAuthQuotaTokenBalanceSource::Upstream,
            used: exhaustion.used(),
            limit: exhaustion.limit(),
            remaining: exhaustion.limit().saturating_sub(exhaustion.used()),
            window_seconds: None,
        }));
    }
    if !meta.status.is_success() {
        return Ok(None);
    }
    let Some(limit) = header_integer(meta, LIMIT_HEADER) else {
        return Ok(None);
    };
    let Some(remaining) = header_integer(meta, REMAINING_HEADER) else {
        return Ok(None);
    };
    if limit == 0 || remaining > limit {
        return Ok(None);
    }
    Ok(Some(OAuthQuotaTokenBalance {
        source: OAuthQuotaTokenBalanceSource::Upstream,
        used: limit - remaining,
        limit,
        remaining,
        window_seconds: None,
    }))
}

fn header_integer(meta: &UpstreamResponseMeta, name: &'static str) -> Option<u64> {
    let value = meta.headers.get(name)?.to_str().ok()?.trim().parse().ok()?;
    (value <= MAX_SAFE_INTEGER).then_some(value)
}

fn is_free(usage: &OAuthQuotaUsage) -> bool {
    usage
        .subscription_tier
        .as_deref()
        .is_some_and(|tier| tier.trim().eq_ignore_ascii_case("free"))
}
