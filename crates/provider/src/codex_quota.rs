use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, header};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError,
    oauth_quota::{
        OAuthQuotaQueryPlan, OAuthQuotaRateLimit, OAuthQuotaResetCredit, OAuthQuotaResetCredits,
        OAuthQuotaResetResult, OAuthQuotaUsage, OAuthQuotaWindow,
    },
};

const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const RESET_CREDITS_URL: &str = "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";
const RESET_URL: &str = "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume";

pub(crate) fn query_plan(token: &OAuthTokenMaterial) -> Result<OAuthQuotaQueryPlan, ProviderError> {
    let headers = quota_headers(token)?;
    Ok(OAuthQuotaQueryPlan::new(
        request(Method::GET, USAGE_URL, headers.clone(), Vec::new())?,
        request(Method::GET, RESET_CREDITS_URL, headers, Vec::new())?,
    ))
}

pub(crate) fn reset_plan(
    token: &OAuthTokenMaterial,
    redeem_request_id: &str,
) -> Result<OAuthRequestPlan, ProviderError> {
    if redeem_request_id.trim().is_empty() {
        return Err(ProviderError::InvalidCredential(
            "Codex quota redeem request id is required".into(),
        ));
    }
    let mut headers = quota_headers(token)?;
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let body = serde_json::to_vec(&serde_json::json!({
        "redeem_request_id": redeem_request_id,
    }))
    .map_err(|_| ProviderError::InvalidCredential("Codex quota reset body is invalid".into()))?;
    request(Method::POST, RESET_URL, headers, body)
}

pub(crate) fn parse_usage(body: &[u8]) -> Result<OAuthQuotaUsage, ProviderError> {
    let payload = serde_json::from_slice::<UsagePayload>(body)
        .map_err(|_| invalid_response("Codex quota usage response is invalid"))?;
    let rate_limit = payload.rate_limit.map(parse_rate_limit).transpose()?;
    let reset_credits = payload
        .rate_limit_reset_credits
        .as_ref()
        .map(parse_reset_credits_value)
        .transpose()?
        .flatten();
    Ok(OAuthQuotaUsage {
        rate_limit,
        reset_credits,
    })
}

pub(crate) fn parse_reset_credits(
    body: &[u8],
) -> Result<Option<OAuthQuotaResetCredits>, ProviderError> {
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|_| invalid_response("Codex quota reset credit response is invalid"))?;
    parse_reset_credits_value(&value)
}

pub(crate) fn parse_reset_result(body: &[u8]) -> Result<OAuthQuotaResetResult, ProviderError> {
    #[derive(Deserialize)]
    struct Payload {
        windows_reset: u32,
    }
    let payload = serde_json::from_slice::<Payload>(body)
        .map_err(|_| invalid_response("Codex quota reset response is invalid"))?;
    if payload.windows_reset == 0 {
        return Err(invalid_response(
            "Codex quota reset response did not reset a window",
        ));
    }
    Ok(OAuthQuotaResetResult {
        windows_reset: payload.windows_reset,
    })
}

fn quota_headers(token: &OAuthTokenMaterial) -> Result<HeaderMap, ProviderError> {
    if token.provider() != ProviderKind::Codex {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Codex quota".into(),
        ));
    }
    let account_id = token.account_id().ok_or_else(|| {
        ProviderError::InvalidCredential("Codex OAuth account id is required for quota".into())
    })?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token.access_token())).map_err(|_| {
            ProviderError::InvalidCredential("invalid OAuth access token header".into())
        })?,
    );
    headers.insert(
        "chatgpt-account-id",
        HeaderValue::from_str(account_id).map_err(|_| {
            ProviderError::InvalidCredential("invalid Codex OAuth account id header".into())
        })?,
    );
    for (name, value) in [
        ("openai-beta", "codex-1"),
        ("oai-language", "zh-CN"),
        ("originator", "Codex Desktop"),
        ("sec-fetch-site", "none"),
        ("sec-fetch-mode", "no-cors"),
        ("sec-fetch-dest", "empty"),
        ("priority", "u=4, i"),
    ] {
        headers.insert(
            http::header::HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    Ok(headers)
}

fn request(
    method: Method,
    url: &'static str,
    headers: HeaderMap,
    body: Vec<u8>,
) -> Result<OAuthRequestPlan, ProviderError> {
    Ok(OAuthRequestPlan {
        method,
        url: Url::parse(url).map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers,
        body,
    })
}

#[derive(Deserialize)]
struct UsagePayload {
    rate_limit: Option<RateLimitPayload>,
    rate_limit_reset_credits: Option<Value>,
}

#[derive(Deserialize)]
struct RateLimitPayload {
    allowed: bool,
    limit_reached: bool,
    primary_window: Option<WindowPayload>,
    secondary_window: Option<WindowPayload>,
}

#[derive(Deserialize)]
struct WindowPayload {
    used_percent: f64,
    limit_window_seconds: u64,
    reset_after_seconds: u64,
    reset_at: i64,
}

fn parse_rate_limit(value: RateLimitPayload) -> Result<OAuthQuotaRateLimit, ProviderError> {
    Ok(OAuthQuotaRateLimit {
        allowed: value.allowed,
        limit_reached: value.limit_reached,
        primary_window: value.primary_window.map(parse_window).transpose()?,
        secondary_window: value.secondary_window.map(parse_window).transpose()?,
    })
}

fn parse_window(value: WindowPayload) -> Result<OAuthQuotaWindow, ProviderError> {
    if !value.used_percent.is_finite() || value.used_percent < 0.0 {
        return Err(invalid_response("Codex quota percentage is invalid"));
    }
    Ok(OAuthQuotaWindow {
        used_percent: value.used_percent,
        limit_window_seconds: value.limit_window_seconds,
        reset_after_seconds: value.reset_after_seconds,
        reset_at: value.reset_at,
    })
}

fn parse_reset_credits_value(
    value: &Value,
) -> Result<Option<OAuthQuotaResetCredits>, ProviderError> {
    if value.is_null() {
        return Ok(None);
    }
    let (count, records) = match value {
        Value::Array(records) => (None, Some(records)),
        Value::Object(object) => {
            let count = object
                .get("available_count")
                .or_else(|| object.get("availableCount"))
                .and_then(parse_available_count);
            let records = ["credits", "rate_limit_reset_credits", "items", "data"]
                .into_iter()
                .find_map(|key| object.get(key).and_then(Value::as_array));
            (count, records)
        }
        _ => return Err(invalid_response("Codex quota reset credits are invalid")),
    };
    if count.is_none() && records.is_none() {
        return Ok(None);
    }
    let mut available_records = 0_u32;
    let mut credits = Vec::new();
    for record in records.into_iter().flatten() {
        let Some(object) = record.as_object() else {
            continue;
        };
        let reset_type = text_field(object, "reset_type", "resetType");
        if reset_type.is_some_and(|value| !value.eq_ignore_ascii_case("codex_rate_limits")) {
            continue;
        }
        if object
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.eq_ignore_ascii_case("available"))
        {
            continue;
        }
        available_records = available_records.saturating_add(1);
        if let Some(expires_at) = text_field(object, "expires_at", "expiresAt") {
            credits.push(OAuthQuotaResetCredit {
                expires_at: expires_at.to_owned(),
            });
        }
    }
    Ok(Some(OAuthQuotaResetCredits {
        available_count: count.unwrap_or(available_records),
        credits,
    }))
}

fn parse_available_count(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse().ok())
}

fn text_field<'a>(
    object: &'a serde_json::Map<String, Value>,
    snake: &str,
    camel: &str,
) -> Option<&'a str> {
    object
        .get(snake)
        .or_else(|| object.get(camel))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}

#[cfg(test)]
#[path = "codex_quota_tests.rs"]
mod tests;
