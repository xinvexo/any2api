//! Anthropic Claude Code OAuth usage quota contract.

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError,
    oauth::quota::{
        OAuthQuotaQueryPlan, OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaWindow,
        OAuthQuotaWindowKind,
    },
};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub(crate) fn query_plan(token: &OAuthTokenMaterial) -> Result<OAuthQuotaQueryPlan, ProviderError> {
    if token.provider() != ProviderKind::Claude {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Claude quota".into(),
        ));
    }
    let mut headers = HeaderMap::new();
    let mut authorization = HeaderValue::from_str(&format!("Bearer {}", token.access_token()))
        .map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?;
    authorization.set_sensitive(true);
    headers.insert(header::AUTHORIZATION, authorization);
    super::super::identity::apply_quota_defaults(&mut headers);
    let usage = OAuthRequestPlan {
        method: Method::GET,
        url: Url::parse(USAGE_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers,
        body: Vec::new(),
    };
    Ok(OAuthQuotaQueryPlan::without_reset_credits(usage))
}

pub(crate) fn parse_usage(body: &[u8]) -> Result<OAuthQuotaUsage, ProviderError> {
    let payload = serde_json::from_slice::<UsagePayload>(body)
        .map_err(|_| invalid_response("Claude quota usage response is invalid"))?;
    let mut windows = Vec::with_capacity(4);
    push_window(&mut windows, "five_hour", 18_000, payload.five_hour)?;
    push_window(&mut windows, "seven_day", 604_800, payload.seven_day)?;
    push_window(
        &mut windows,
        "seven_day_sonnet",
        604_800,
        payload.seven_day_sonnet,
    )?;
    push_window(
        &mut windows,
        "seven_day_overage_included",
        604_800,
        payload.seven_day_overage_included,
    )?;
    if windows.is_empty() {
        return Err(invalid_response("Claude quota usage windows are missing"));
    }
    Ok(OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: None,
            limit_reached: None,
            windows,
        }),
        reset_credits: None,
        billing: None,
        token_balance: None,
        subscription_tier: None,
        account_status: None,
    })
}

#[derive(Deserialize)]
struct UsagePayload {
    five_hour: Option<WindowPayload>,
    seven_day: Option<WindowPayload>,
    seven_day_sonnet: Option<WindowPayload>,
    seven_day_overage_included: Option<WindowPayload>,
}

#[derive(Deserialize)]
struct WindowPayload {
    utilization: f64,
    resets_at: String,
}

fn push_window(
    windows: &mut Vec<OAuthQuotaWindow>,
    id: &'static str,
    duration: u64,
    payload: Option<WindowPayload>,
) -> Result<(), ProviderError> {
    let Some(payload) = payload else {
        return Ok(());
    };
    if !payload.utilization.is_finite() || payload.utilization < 0.0 {
        return Err(invalid_response("Claude quota utilization is invalid"));
    }
    let reset_at = OffsetDateTime::parse(payload.resets_at.trim(), &Rfc3339)
        .map(OffsetDateTime::unix_timestamp)
        .map_err(|_| invalid_response("Claude quota reset timestamp is invalid"))?;
    windows.push(OAuthQuotaWindow {
        id: id.to_owned(),
        kind: OAuthQuotaWindowKind::Time,
        used_percent: payload.utilization,
        limit_window_seconds: Some(duration),
        reset_after_seconds: None,
        reset_at: Some(reset_at),
    });
    Ok(())
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}
