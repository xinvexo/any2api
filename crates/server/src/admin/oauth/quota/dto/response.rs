//! Provider-neutral quota response DTOs.

use any2api_runtime::api::{
    OAuthQuotaAccessStatus, OAuthQuotaAccountStatus, OAuthQuotaAuthenticationStatus,
    OAuthQuotaBilling, OAuthQuotaCredits, OAuthQuotaExhaustion, OAuthQuotaRateCard,
    OAuthQuotaRateLimit, OAuthQuotaReachedType, OAuthQuotaResetCredits, OAuthQuotaSnapshot,
    OAuthQuotaTokenBalance, OAuthQuotaTokenBalanceSource, OAuthQuotaWindow, OAuthQuotaWindowKind,
};
use serde::Serialize;

use super::estimate::OAuthQuotaEstimateResponse;

#[derive(Debug, Serialize)]
pub(in crate::admin::oauth::quota) struct OAuthQuotaResponse {
    fetched_at: i64,
    rate_limit: Option<OAuthQuotaRateLimitResponse>,
    credits: Option<OAuthQuotaCreditsResponse>,
    access: Option<OAuthQuotaAccessStatusResponse>,
    reset_credits: Option<OAuthQuotaResetCreditsResponse>,
    billing: Option<OAuthQuotaBillingResponse>,
    token_balance: Option<OAuthQuotaTokenBalanceResponse>,
    subscription_tier: Option<String>,
    account_status: Option<OAuthQuotaAccountStatusResponse>,
    estimates: Vec<OAuthQuotaEstimateResponse>,
    rate_card: Option<OAuthQuotaRateCardResponse>,
}

impl From<OAuthQuotaSnapshot> for OAuthQuotaResponse {
    fn from(snapshot: OAuthQuotaSnapshot) -> Self {
        Self {
            fetched_at: snapshot.fetched_at,
            rate_limit: snapshot.usage.rate_limit.map(Into::into),
            credits: snapshot.usage.credits.map(Into::into),
            access: snapshot.usage.access.map(Into::into),
            reset_credits: snapshot.usage.reset_credits.map(Into::into),
            billing: snapshot.usage.billing.map(Into::into),
            token_balance: snapshot.usage.token_balance.map(Into::into),
            subscription_tier: snapshot.usage.subscription_tier,
            account_status: snapshot.usage.account_status.map(Into::into),
            estimates: snapshot.estimates.into_iter().map(Into::into).collect(),
            rate_card: snapshot.rate_card.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaRateCardResponse {
    id: String,
    credits_per_usd: u64,
}

impl From<OAuthQuotaRateCard> for OAuthQuotaRateCardResponse {
    fn from(value: OAuthQuotaRateCard) -> Self {
        Self {
            id: value.id,
            credits_per_usd: value.credits_per_usd,
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaCreditsResponse {
    has_credits: bool,
    unlimited: bool,
    balance: Option<String>,
}

impl From<OAuthQuotaCredits> for OAuthQuotaCreditsResponse {
    fn from(value: OAuthQuotaCredits) -> Self {
        Self {
            has_credits: value.has_credits,
            unlimited: value.unlimited,
            balance: value.balance,
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaAccessStatusResponse {
    spend_control_reached: Option<bool>,
    reached_type: Option<&'static str>,
}

impl From<OAuthQuotaAccessStatus> for OAuthQuotaAccessStatusResponse {
    fn from(value: OAuthQuotaAccessStatus) -> Self {
        Self {
            spend_control_reached: value.spend_control_reached,
            reached_type: value.reached_type.map(reached_type),
        }
    }
}

fn reached_type(value: OAuthQuotaReachedType) -> &'static str {
    match value {
        OAuthQuotaReachedType::RateLimitReached => "rate_limit_reached",
        OAuthQuotaReachedType::WorkspaceOwnerCreditsDepleted => "workspace_owner_credits_depleted",
        OAuthQuotaReachedType::WorkspaceMemberCreditsDepleted => {
            "workspace_member_credits_depleted"
        }
        OAuthQuotaReachedType::WorkspaceOwnerUsageLimitReached => {
            "workspace_owner_usage_limit_reached"
        }
        OAuthQuotaReachedType::WorkspaceMemberUsageLimitReached => {
            "workspace_member_usage_limit_reached"
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaTokenBalanceResponse {
    source: &'static str,
    used: u64,
    limit: u64,
    remaining: u64,
    window_seconds: Option<u64>,
}

impl From<OAuthQuotaTokenBalance> for OAuthQuotaTokenBalanceResponse {
    fn from(value: OAuthQuotaTokenBalance) -> Self {
        Self {
            source: match value.source {
                OAuthQuotaTokenBalanceSource::Upstream => "upstream",
            },
            used: value.used,
            limit: value.limit,
            remaining: value.remaining,
            window_seconds: value.window_seconds,
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaAccountStatusResponse {
    authentication: &'static str,
    user_blocked_reason: Option<String>,
    team_blocked_reasons: Vec<String>,
    quota_exhaustion: Option<OAuthQuotaExhaustionResponse>,
}

impl From<OAuthQuotaAccountStatus> for OAuthQuotaAccountStatusResponse {
    fn from(value: OAuthQuotaAccountStatus) -> Self {
        Self {
            authentication: match value.authentication {
                OAuthQuotaAuthenticationStatus::Valid => "valid",
            },
            user_blocked_reason: value.user_blocked_reason,
            team_blocked_reasons: value.team_blocked_reasons,
            quota_exhaustion: value.quota_exhaustion.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaExhaustionResponse {
    observed_at: i64,
    used: Option<u64>,
    limit: Option<u64>,
}

impl From<OAuthQuotaExhaustion> for OAuthQuotaExhaustionResponse {
    fn from(value: OAuthQuotaExhaustion) -> Self {
        Self {
            observed_at: value.observed_at,
            used: value.used,
            limit: value.limit,
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthQuotaBillingResponse {
    currency: String,
    prepaid_balance_minor: Option<i64>,
    on_demand_used_minor: Option<i64>,
    on_demand_cap_minor: Option<i64>,
    is_unified_billing_user: Option<bool>,
}

impl From<OAuthQuotaBilling> for OAuthQuotaBillingResponse {
    fn from(value: OAuthQuotaBilling) -> Self {
        Self {
            currency: value.currency,
            prepaid_balance_minor: value.prepaid_balance_minor,
            on_demand_used_minor: value.on_demand_used_minor,
            on_demand_cap_minor: value.on_demand_cap_minor,
            is_unified_billing_user: value.is_unified_billing_user,
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
    id: String,
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
            kind: window_kind(value.kind),
            used_percent: value.used_percent,
            limit_window_seconds: value.limit_window_seconds,
            reset_after_seconds: value.reset_after_seconds,
            reset_at: value.reset_at,
        }
    }
}

fn window_kind(value: OAuthQuotaWindowKind) -> &'static str {
    match value {
        OAuthQuotaWindowKind::Time => "time",
        OAuthQuotaWindowKind::Credits => "credits",
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

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
