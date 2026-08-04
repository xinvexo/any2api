//! Provider-neutral quota response DTOs.

use any2api_runtime::api::{
    OAuthQuotaAccountStatus, OAuthQuotaAuthenticationStatus, OAuthQuotaBilling,
    OAuthQuotaExhaustion, OAuthQuotaRateLimit, OAuthQuotaResetCredits, OAuthQuotaResetOutcome,
    OAuthQuotaSnapshot, OAuthQuotaTokenBalance, OAuthQuotaTokenBalanceSource, OAuthQuotaWindow,
    OAuthQuotaWindowKind,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct OAuthQuotaResponse {
    fetched_at: i64,
    rate_limit: Option<OAuthQuotaRateLimitResponse>,
    reset_credits: Option<OAuthQuotaResetCreditsResponse>,
    billing: Option<OAuthQuotaBillingResponse>,
    token_balance: Option<OAuthQuotaTokenBalanceResponse>,
    subscription_tier: Option<String>,
    account_status: Option<OAuthQuotaAccountStatusResponse>,
}

impl From<OAuthQuotaSnapshot> for OAuthQuotaResponse {
    fn from(snapshot: OAuthQuotaSnapshot) -> Self {
        Self {
            fetched_at: snapshot.fetched_at,
            rate_limit: snapshot.usage.rate_limit.map(Into::into),
            reset_credits: snapshot.usage.reset_credits.map(Into::into),
            billing: snapshot.usage.billing.map(Into::into),
            token_balance: snapshot.usage.token_balance.map(Into::into),
            subscription_tier: snapshot.usage.subscription_tier,
            account_status: snapshot.usage.account_status.map(Into::into),
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
            kind: match value.kind {
                OAuthQuotaWindowKind::Time => "time",
                OAuthQuotaWindowKind::Credits => "credits",
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

#[cfg(test)]
mod tests {
    use super::*;
    use any2api_runtime::api::OAuthQuotaUsage;

    #[test]
    fn serializes_grok_billing_and_subscription_without_secrets() {
        let response = OAuthQuotaResponse::from(OAuthQuotaSnapshot {
            fetched_at: 1_900_000_000,
            usage: OAuthQuotaUsage {
                rate_limit: None,
                reset_credits: None,
                billing: Some(OAuthQuotaBilling {
                    currency: "USD".to_owned(),
                    prepaid_balance_minor: Some(-2500),
                    on_demand_used_minor: Some(125),
                    on_demand_cap_minor: Some(5000),
                    is_unified_billing_user: Some(true),
                }),
                token_balance: Some(OAuthQuotaTokenBalance {
                    source: OAuthQuotaTokenBalanceSource::Upstream,
                    used: 1_065_387,
                    limit: 1_000_000,
                    remaining: 0,
                    window_seconds: None,
                }),
                subscription_tier: Some("SuperGrokPro".into()),
                account_status: Some(OAuthQuotaAccountStatus {
                    authentication: OAuthQuotaAuthenticationStatus::Valid,
                    user_blocked_reason: Some("BLOCKED_REASON_BILLING".into()),
                    team_blocked_reasons: vec!["BLOCKED_REASON_NO_LOGS".into()],
                    quota_exhaustion: Some(OAuthQuotaExhaustion {
                        observed_at: 1_900_000_000,
                        used: Some(1_065_387),
                        limit: Some(1_000_000),
                    }),
                }),
            },
        });
        let value = serde_json::to_value(response).expect("quota response");

        assert_eq!(value["subscription_tier"], "SuperGrokPro");
        assert_eq!(value["billing"]["currency"], "USD");
        assert_eq!(value["billing"]["prepaid_balance_minor"], -2500);
        assert_eq!(value["billing"]["on_demand_used_minor"], 125);
        assert_eq!(value["billing"]["on_demand_cap_minor"], 5000);
        assert_eq!(value["token_balance"]["source"], "upstream");
        assert_eq!(value["token_balance"]["used"], 1_065_387);
        assert_eq!(value["token_balance"]["limit"], 1_000_000);
        assert_eq!(value["token_balance"]["remaining"], 0);
        assert!(value["token_balance"]["window_seconds"].is_null());
        assert_eq!(value["account_status"]["authentication"], "valid");
        assert_eq!(
            value["account_status"]["user_blocked_reason"],
            "BLOCKED_REASON_BILLING"
        );
        assert_eq!(
            value["account_status"]["quota_exhaustion"]["used"],
            1_065_387
        );
    }
}
