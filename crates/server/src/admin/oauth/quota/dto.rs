//! Provider-neutral quota response DTOs.

use any2api_runtime::api::{
    OAuthQuotaRateLimit, OAuthQuotaResetCredits, OAuthQuotaResetOutcome, OAuthQuotaSnapshot,
    OAuthQuotaWindow, OAuthQuotaWindowKind,
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
    allowed: Option<bool>,
    limit_reached: Option<bool>,
    windows: Vec<OAuthQuotaWindowResponse>,
}

impl From<OAuthQuotaRateLimit> for OAuthQuotaRateLimitResponse {
    fn from(value: OAuthQuotaRateLimit) -> Self {
        Self {
            allowed: value.allowed,
            limit_reached: value.limit_reached,
            windows: value.windows.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaWindowResponse {
    id: &'static str,
    kind: &'static str,
    used_percent: f64,
    limit_window_seconds: Option<u64>,
    reset_after_seconds: Option<u64>,
    reset_at: Option<i64>,
}

impl From<OAuthQuotaWindow> for OAuthQuotaWindowResponse {
    fn from(value: OAuthQuotaWindow) -> Self {
        Self {
            id: value.id,
            kind: match value.kind {
                OAuthQuotaWindowKind::Time => "time",
                OAuthQuotaWindowKind::Credits => "credits",
                OAuthQuotaWindowKind::Requests => "requests",
                OAuthQuotaWindowKind::Tokens => "tokens",
            },
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
