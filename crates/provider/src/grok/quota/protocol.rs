//! xAI CLI subscription billing quota contract.

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use crate::grok::oauth;
use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError,
    api::UpstreamResponseMeta,
    oauth::quota::{
        OAuthQuotaQueryPlan, OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaUsageParse,
        OAuthQuotaWindow, OAuthQuotaWindowKind,
    },
};

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const PROBE_URL: &str = "https://cli-chat-proxy.grok.com/v1/responses";
const PROBE_MODEL: &str = "grok-4.5";

pub(crate) fn query_plan(token: &OAuthTokenMaterial) -> Result<OAuthQuotaQueryPlan, ProviderError> {
    if token.provider() != ProviderKind::Grok {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Grok quota".into(),
        ));
    }
    let mut billing_headers = oauth::credential_headers(token)?.headers;
    billing_headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    billing_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let usage = OAuthRequestPlan {
        method: Method::GET,
        url: Url::parse(BILLING_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers: billing_headers.clone(),
        body: Vec::new(),
    };
    billing_headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    let usage_probe = OAuthRequestPlan {
        method: Method::POST,
        url: Url::parse(PROBE_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers: billing_headers,
        body: serde_json::to_vec(&serde_json::json!({
            "model": PROBE_MODEL,
            "input": "hi",
            "stream": true,
        }))
        .map_err(|_| invalid_response("Grok quota probe request is invalid"))?,
    };
    Ok(OAuthQuotaQueryPlan::with_usage_probe(usage, usage_probe))
}

pub(crate) fn parse_usage(body: &[u8]) -> Result<OAuthQuotaUsageParse, ProviderError> {
    let payload = serde_json::from_slice::<BillingPayload>(body)
        .map_err(|_| invalid_response("Grok billing response is invalid"))?;
    let (end, duration) = parse_period(&payload.config.current_period)?;
    let Some(used_percent) = payload.config.credit_usage_percent else {
        return Ok(OAuthQuotaUsageParse::ProbeRequired);
    };
    if !used_percent.is_finite() || used_percent < 0.0 {
        return Err(invalid_response("Grok billing percentage is invalid"));
    }
    let reset_after_seconds = u64::try_from(end.saturating_sub(unix_now())).unwrap_or_default();
    let limit_reached = used_percent >= 100.0;
    Ok(OAuthQuotaUsageParse::Complete(OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: Some(!limit_reached),
            limit_reached: Some(limit_reached),
            windows: vec![OAuthQuotaWindow {
                id: "weekly_credits",
                kind: OAuthQuotaWindowKind::Credits,
                used_percent,
                limit_window_seconds: Some(duration),
                reset_after_seconds: Some(reset_after_seconds),
                reset_at: Some(end),
            }],
        }),
        reset_credits: None,
    }))
}

pub(crate) fn parse_probe(meta: &UpstreamResponseMeta) -> Result<OAuthQuotaUsage, ProviderError> {
    let requests = parse_header_window(
        &meta.headers,
        "requests",
        "requests",
        OAuthQuotaWindowKind::Requests,
    )?;
    let tokens = parse_header_window(
        &meta.headers,
        "tokens",
        "tokens",
        OAuthQuotaWindowKind::Tokens,
    )?;
    if requests.is_none() && tokens.is_none() && meta.status != StatusCode::TOO_MANY_REQUESTS {
        return Err(invalid_response("Grok quota probe headers are missing"));
    }
    let limit_reached = meta.status == StatusCode::TOO_MANY_REQUESTS
        || requests
            .iter()
            .chain(tokens.iter())
            .any(|window| window.used_percent >= 100.0);
    let windows = [requests, tokens].into_iter().flatten().collect();
    Ok(OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: Some(!limit_reached),
            limit_reached: Some(limit_reached),
            windows,
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
    credit_usage_percent: Option<f64>,
}

#[derive(Deserialize)]
struct BillingPeriod {
    #[serde(rename = "type")]
    kind: Option<String>,
    start: String,
    end: String,
}

fn parse_period(period: &BillingPeriod) -> Result<(i64, u64), ProviderError> {
    if period.kind.as_deref().is_some_and(|kind| {
        !kind.eq_ignore_ascii_case("weekly")
            && !kind.eq_ignore_ascii_case("usage_period_type_weekly")
    }) {
        return Err(invalid_response("Grok billing period is not weekly"));
    }
    let start = parse_timestamp(&period.start)?;
    let end = parse_timestamp(&period.end)?;
    let duration = end
        .checked_sub(start)
        .filter(|duration| *duration > 0)
        .and_then(|duration| u64::try_from(duration).ok())
        .ok_or_else(|| invalid_response("Grok billing period is invalid"))?;
    Ok((end, duration))
}

fn parse_header_window(
    headers: &HeaderMap,
    dimension: &str,
    id: &'static str,
    kind: OAuthQuotaWindowKind,
) -> Result<Option<OAuthQuotaWindow>, ProviderError> {
    let limit = parse_integer_header(headers, &format!("x-ratelimit-limit-{dimension}"))?;
    let remaining = parse_integer_header(headers, &format!("x-ratelimit-remaining-{dimension}"))?;
    let (Some(limit), Some(remaining)) = (limit, remaining) else {
        return if limit.is_none() && remaining.is_none() {
            Ok(None)
        } else {
            Err(invalid_response("Grok quota limit headers are incomplete"))
        };
    };
    if limit == 0 || remaining > limit {
        return Err(invalid_response("Grok quota limit headers are invalid"));
    }
    let reset_at = parse_optional_reset(headers, &format!("x-ratelimit-reset-{dimension}"))?;
    let reset_after_seconds = reset_at
        .map(|reset_at| u64::try_from(reset_at.saturating_sub(unix_now())).unwrap_or_default());
    Ok(Some(OAuthQuotaWindow {
        id,
        kind,
        used_percent: (limit - remaining) as f64 / limit as f64 * 100.0,
        limit_window_seconds: None,
        reset_after_seconds,
        reset_at,
    }))
}

fn parse_integer_header(headers: &HeaderMap, name: &str) -> Result<Option<u64>, ProviderError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    value
        .to_str()
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .map(Some)
        .ok_or_else(|| invalid_response("Grok quota integer header is invalid"))
}

fn parse_optional_reset(headers: &HeaderMap, name: &str) -> Result<Option<i64>, ProviderError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map(str::trim)
        .map_err(|_| invalid_response("Grok quota reset header is invalid"))?;
    let reset_at = value
        .parse::<i64>()
        .ok()
        .map(|timestamp| {
            if timestamp > 1_000_000_000_000 {
                timestamp / 1_000
            } else {
                timestamp
            }
        })
        .or_else(|| {
            OffsetDateTime::parse(value, &Rfc3339)
                .ok()
                .map(OffsetDateTime::unix_timestamp)
        })
        .filter(|timestamp| *timestamp > 0)
        .ok_or_else(|| invalid_response("Grok quota reset header is invalid"))?;
    Ok(Some(reset_at))
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
