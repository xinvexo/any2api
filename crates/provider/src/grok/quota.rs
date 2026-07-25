//! xAI CLI subscription billing quota contract.

use any2api_domain::ProviderKind;
use http::{HeaderValue, Method, header};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use super::oauth;
use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError,
    oauth_quota::{OAuthQuotaQueryPlan, OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaWindow},
};

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";

pub(crate) fn query_plan(token: &OAuthTokenMaterial) -> Result<OAuthQuotaQueryPlan, ProviderError> {
    if token.provider() != ProviderKind::Grok {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Grok quota".into(),
        ));
    }
    let mut headers = oauth::credential_headers(token)?.headers;
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let usage = OAuthRequestPlan {
        method: Method::GET,
        url: Url::parse(BILLING_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers,
        body: Vec::new(),
    };
    Ok(OAuthQuotaQueryPlan::without_reset_credits(usage))
}

pub(crate) fn parse_usage(body: &[u8]) -> Result<OAuthQuotaUsage, ProviderError> {
    let payload = serde_json::from_slice::<BillingPayload>(body)
        .map_err(|_| invalid_response("Grok billing response is invalid"))?;
    if payload
        .config
        .current_period
        .kind
        .as_deref()
        .is_some_and(|kind| !kind.eq_ignore_ascii_case("weekly"))
    {
        return Err(invalid_response("Grok billing period is not weekly"));
    }
    let used_percent = payload.config.credit_usage_percent;
    if !used_percent.is_finite() || used_percent < 0.0 {
        return Err(invalid_response("Grok billing percentage is invalid"));
    }
    let start = parse_timestamp(&payload.config.current_period.start)?;
    let end = parse_timestamp(&payload.config.current_period.end)?;
    let duration = end
        .checked_sub(start)
        .filter(|duration| *duration > 0)
        .and_then(|duration| u64::try_from(duration).ok())
        .ok_or_else(|| invalid_response("Grok billing period is invalid"))?;
    let reset_after_seconds = u64::try_from(end.saturating_sub(unix_now())).unwrap_or_default();
    let limit_reached = used_percent >= 100.0;
    Ok(OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: !limit_reached,
            limit_reached,
            primary_window: Some(OAuthQuotaWindow {
                used_percent,
                limit_window_seconds: duration,
                reset_after_seconds,
                reset_at: end,
            }),
            secondary_window: None,
        }),
        reset_credits: None,
    })
}

#[derive(Deserialize)]
struct BillingPayload {
    config: BillingConfig,
}

#[derive(Deserialize)]
struct BillingConfig {
    #[serde(rename = "currentPeriod")]
    current_period: BillingPeriod,
    #[serde(rename = "creditUsagePercent")]
    credit_usage_percent: f64,
}

#[derive(Deserialize)]
struct BillingPeriod {
    #[serde(rename = "type")]
    kind: Option<String>,
    start: String,
    end: String,
}

fn parse_timestamp(value: &str) -> Result<i64, ProviderError> {
    OffsetDateTime::parse(value.trim(), &Rfc3339)
        .map(OffsetDateTime::unix_timestamp)
        .map_err(|_| invalid_response("Grok billing timestamp is invalid"))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}
