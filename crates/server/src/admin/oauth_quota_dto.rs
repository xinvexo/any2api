use any2api_runtime::api::{
    OAuthQuotaRateLimit, OAuthQuotaResetCredits, OAuthQuotaResetOutcome, OAuthQuotaSnapshot,
    OAuthQuotaWindow,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct OAuthQuotaResponse {
    fetched_at: i64,
    rate_limit: Option<OAuthQuotaRateLimitResponse>,
    reset_credits: Option<OAuthQuotaResetCreditsResponse>,
}

impl From<OAuthQuotaSnapshot> for OAuthQuotaResponse {
    fn from(snapshot: OAuthQuotaSnapshot) -> Self {
        Self {
            fetched_at: snapshot.fetched_at,
            rate_limit: snapshot.usage.rate_limit.map(Into::into),
            reset_credits: snapshot.usage.reset_credits.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaRateLimitResponse {
    allowed: bool,
    limit_reached: bool,
    primary_window: Option<OAuthQuotaWindowResponse>,
    secondary_window: Option<OAuthQuotaWindowResponse>,
}

impl From<OAuthQuotaRateLimit> for OAuthQuotaRateLimitResponse {
    fn from(value: OAuthQuotaRateLimit) -> Self {
        Self {
            allowed: value.allowed,
            limit_reached: value.limit_reached,
            primary_window: value.primary_window.map(Into::into),
            secondary_window: value.secondary_window.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaWindowResponse {
    used_percent: f64,
    limit_window_seconds: u64,
    reset_after_seconds: u64,
    reset_at: i64,
}

impl From<OAuthQuotaWindow> for OAuthQuotaWindowResponse {
    fn from(value: OAuthQuotaWindow) -> Self {
        Self {
            used_percent: value.used_percent,
            limit_window_seconds: value.limit_window_seconds,
            reset_after_seconds: value.reset_after_seconds,
            reset_at: value.reset_at,
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaResetCreditsResponse {
    available_count: u32,
    expires_at: Vec<String>,
}

impl From<OAuthQuotaResetCredits> for OAuthQuotaResetCreditsResponse {
    fn from(value: OAuthQuotaResetCredits) -> Self {
        Self {
            available_count: value.available_count,
            expires_at: value
                .credits
                .into_iter()
                .map(|credit| credit.expires_at)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct OAuthQuotaResetResponse {
    windows_reset: u32,
}

impl From<OAuthQuotaResetOutcome> for OAuthQuotaResetResponse {
    fn from(value: OAuthQuotaResetOutcome) -> Self {
        Self {
            windows_reset: value.windows_reset,
        }
    }
}
