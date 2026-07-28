//! xAI CLI subscription billing and tier contracts.

use any2api_domain::ProviderKind;
use http::{HeaderValue, Method, header};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use url::Url;

use crate::grok::oauth;
use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError,
    oauth::quota::{
        OAuthLocalTokenQuotaPolicy, OAuthQuotaBilling, OAuthQuotaQueryPlan, OAuthQuotaRateLimit,
        OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
    },
};

const BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";
const SUBSCRIPTION_URL: &str = "https://cli-chat-proxy.grok.com/v1/user?include=subscription";
const MAX_SAFE_INTEGER_MINOR: u64 = 9_007_199_254_740_991;
const FREE_TOKEN_LIMIT: u64 = 1_000_000;
const FREE_TOKEN_WINDOW_SECONDS: u64 = 86_400;

pub(crate) fn query_plan(token: &OAuthTokenMaterial) -> Result<OAuthQuotaQueryPlan, ProviderError> {
    if token.provider() != ProviderKind::Grok {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Grok quota".into(),
        ));
    }
    let account_id = token.account_id().ok_or_else(|| {
        ProviderError::InvalidCredential("Grok OAuth subject is required for quota".into())
    })?;
    let mut headers = oauth::credential_headers(token)?.headers;
    headers.insert("x-xai-token-auth", HeaderValue::from_static("xai-grok-cli"));
    headers.insert(
        "x-authenticateresponse",
        HeaderValue::from_static("authenticate-response"),
    );
    headers.insert("x-grok-client-version", HeaderValue::from_static("0.2.112"));
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("grok-shell/0.2.112 (macos; aarch64)"),
    );
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(
        "x-userid",
        HeaderValue::from_str(account_id).map_err(|_| {
            ProviderError::InvalidCredential("Grok OAuth subject is invalid".into())
        })?,
    );
    headers.insert(
        "x-grok-client-mode",
        HeaderValue::from_static("interactive"),
    );
    let usage = get(BILLING_URL, headers.clone())?;
    let supplement = get(SUBSCRIPTION_URL, headers)?;
    Ok(OAuthQuotaQueryPlan::with_supplement(usage, supplement))
}

pub(crate) fn parse_usage(body: &[u8]) -> Result<OAuthQuotaUsage, ProviderError> {
    let payload = serde_json::from_slice::<BillingPayload>(body)
        .map_err(|_| invalid_response("Grok billing response is invalid"))?;
    let config = payload
        .config
        .ok_or_else(|| invalid_response("Grok billing config is missing"))?;
    let monthly_limit = parse_optional_cent(config.monthly_limit.as_ref())?;
    let legacy_used = parse_optional_cent(config.used.as_ref())?;
    let prepaid_balance_minor = parse_optional_cent(config.prepaid_balance.as_ref())?;
    let on_demand_used_minor = parse_optional_cent(config.on_demand_used.as_ref())?;
    let on_demand_cap_minor = parse_optional_cent(config.on_demand_cap.as_ref())?;
    let period = parse_period(&config)?;
    let used_percent = parse_used_percent(config.credit_usage_percent, monthly_limit, legacy_used)?;
    let rate_limit = match used_percent {
        Some(used_percent) => {
            let period =
                period.ok_or_else(|| invalid_response("Grok billing period is missing"))?;
            let limit_reached = used_percent >= 100.0;
            Some(OAuthQuotaRateLimit {
                allowed: Some(!limit_reached),
                limit_reached: Some(limit_reached),
                windows: vec![OAuthQuotaWindow {
                    id: period.id,
                    kind: OAuthQuotaWindowKind::Credits,
                    used_percent,
                    limit_window_seconds: Some(period.duration),
                    reset_after_seconds: Some(
                        u64::try_from(period.end.saturating_sub(unix_now())).unwrap_or_default(),
                    ),
                    reset_at: Some(period.end),
                }],
            })
        }
        None => None,
    };
    let has_billing = prepaid_balance_minor.is_some()
        || on_demand_used_minor.is_some()
        || on_demand_cap_minor.is_some()
        || config.is_unified_billing_user.is_some();
    let billing = has_billing.then_some(OAuthQuotaBilling {
        currency: "USD",
        prepaid_balance_minor,
        on_demand_used_minor,
        on_demand_cap_minor,
        is_unified_billing_user: config.is_unified_billing_user,
    });
    let subscription_tier = non_empty(payload.subscription_tier);
    if rate_limit.is_none() && billing.is_none() && period.is_none() && subscription_tier.is_none()
    {
        return Err(invalid_response("Grok billing fields are missing"));
    }
    Ok(OAuthQuotaUsage {
        rate_limit,
        reset_credits: None,
        billing,
        token_balance: None,
        subscription_tier,
        account_status: None,
    })
}

pub(crate) fn local_token_quota_policy(
    usage: &OAuthQuotaUsage,
) -> Option<OAuthLocalTokenQuotaPolicy> {
    usage
        .subscription_tier
        .as_deref()
        .is_some_and(|tier| tier.trim().eq_ignore_ascii_case("free"))
        .then_some(OAuthLocalTokenQuotaPolicy {
            limit: FREE_TOKEN_LIMIT,
            window_seconds: FREE_TOKEN_WINDOW_SECONDS,
        })
}

fn get(url: &str, headers: http::HeaderMap) -> Result<OAuthRequestPlan, ProviderError> {
    Ok(OAuthRequestPlan {
        method: Method::GET,
        url: Url::parse(url).map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers,
        body: Vec::new(),
    })
}

#[derive(Deserialize)]
struct BillingPayload {
    config: Option<BillingConfig>,
    #[serde(rename = "subscriptionTier")]
    subscription_tier: Option<String>,
}

#[derive(Deserialize)]
struct BillingConfig {
    #[serde(rename = "currentPeriod")]
    current_period: Option<BillingPeriod>,
    #[serde(rename = "creditUsagePercent")]
    credit_usage_percent: Option<f64>,
    #[serde(rename = "monthlyLimit")]
    monthly_limit: Option<Cent>,
    used: Option<Cent>,
    #[serde(rename = "onDemandCap")]
    on_demand_cap: Option<Cent>,
    #[serde(rename = "onDemandUsed")]
    on_demand_used: Option<Cent>,
    #[serde(rename = "prepaidBalance")]
    prepaid_balance: Option<Cent>,
    #[serde(rename = "isUnifiedBillingUser")]
    is_unified_billing_user: Option<bool>,
    #[serde(rename = "billingPeriodStart")]
    billing_period_start: Option<String>,
    #[serde(rename = "billingPeriodEnd")]
    billing_period_end: Option<String>,
}

#[derive(Deserialize)]
struct BillingPeriod {
    #[serde(rename = "type")]
    kind: Option<String>,
    start: Option<String>,
    end: Option<String>,
}

#[derive(Deserialize)]
struct Cent {
    #[serde(default)]
    val: Option<CentValue>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CentValue {
    Integer(i64),
    Text(String),
}

#[derive(Clone, Copy)]
struct ParsedPeriod {
    id: &'static str,
    duration: u64,
    end: i64,
}

fn parse_used_percent(
    authoritative: Option<f64>,
    monthly_limit: Option<i64>,
    legacy_used: Option<i64>,
) -> Result<Option<f64>, ProviderError> {
    if let Some(value) = authoritative {
        if !value.is_finite() || value < 0.0 {
            return Err(invalid_response("Grok billing percentage is invalid"));
        }
        return Ok(Some(value.min(100.0)));
    }
    match (monthly_limit, legacy_used) {
        (Some(limit), Some(used)) if limit > 0 && used >= 0 => {
            Ok(Some((used as f64 / limit as f64 * 100.0).min(100.0)))
        }
        (Some(limit), _) if limit < 0 => {
            Err(invalid_response("Grok billing monthly limit is invalid"))
        }
        (_, Some(used)) if used < 0 => Err(invalid_response("Grok billing usage is invalid")),
        _ => Ok(None),
    }
}

fn parse_period(config: &BillingConfig) -> Result<Option<ParsedPeriod>, ProviderError> {
    let (kind, start, end) = if let Some(period) = &config.current_period {
        let kind = match period.kind.as_deref().map(str::trim) {
            Some(value) if value.eq_ignore_ascii_case("WEEKLY") || value.ends_with("_WEEKLY") => {
                "weekly_credits"
            }
            Some(value) if value.eq_ignore_ascii_case("MONTHLY") || value.ends_with("_MONTHLY") => {
                "monthly_credits"
            }
            _ => return Err(invalid_response("Grok billing period type is invalid")),
        };
        (
            kind,
            period
                .start
                .as_deref()
                .or(config.billing_period_start.as_deref()),
            period
                .end
                .as_deref()
                .or(config.billing_period_end.as_deref()),
        )
    } else if config.billing_period_start.is_some() || config.billing_period_end.is_some() {
        (
            "monthly_credits",
            config.billing_period_start.as_deref(),
            config.billing_period_end.as_deref(),
        )
    } else {
        return Ok(None);
    };
    let start = parse_timestamp(
        start.ok_or_else(|| invalid_response("Grok billing period is incomplete"))?,
    )?;
    let end =
        parse_timestamp(end.ok_or_else(|| invalid_response("Grok billing period is incomplete"))?)?;
    let duration = end
        .checked_sub(start)
        .filter(|duration| *duration > 0)
        .and_then(|duration| u64::try_from(duration).ok())
        .ok_or_else(|| invalid_response("Grok billing period is invalid"))?;
    Ok(Some(ParsedPeriod {
        id: kind,
        duration,
        end,
    }))
}

fn parse_optional_cent(value: Option<&Cent>) -> Result<Option<i64>, ProviderError> {
    value.map(parse_cent).transpose()
}

fn parse_cent(value: &Cent) -> Result<i64, ProviderError> {
    let value = match value.val.as_ref() {
        None => 0,
        Some(CentValue::Integer(value)) => *value,
        Some(CentValue::Text(value)) => value
            .trim()
            .parse()
            .map_err(|_| invalid_response("Grok billing amount is invalid"))?,
    };
    if value.unsigned_abs() > MAX_SAFE_INTEGER_MINOR {
        return Err(invalid_response("Grok billing amount is unsafe"));
    }
    Ok(value)
}

fn parse_timestamp(value: &str) -> Result<i64, ProviderError> {
    OffsetDateTime::parse(value.trim(), &Rfc3339)
        .map(OffsetDateTime::unix_timestamp)
        .map_err(|_| invalid_response("Grok billing timestamp is invalid"))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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
