use super::*;
use any2api_runtime::api::{OAuthQuotaUsage, OAuthQuotaWindowKind};

#[test]
fn serializes_real_codex_credits_access_and_usd_estimate() {
    let response = OAuthQuotaResponse::from(OAuthQuotaSnapshot {
        fetched_at: 1_900_000_000,
        usage: OAuthQuotaUsage {
            rate_limit: None,
            credits: Some(OAuthQuotaCredits {
                has_credits: true,
                unlimited: false,
                balance: Some("17.50".to_owned()),
            }),
            access: Some(OAuthQuotaAccessStatus {
                spend_control_reached: Some(false),
                reached_type: Some(OAuthQuotaReachedType::RateLimitReached),
            }),
            reset_credits: None,
            billing: None,
            token_balance: None,
            subscription_tier: None,
            account_status: None,
        },
        usd_estimates: vec![OAuthQuotaUsdEstimate {
            window_id: "primary".to_owned(),
            window_kind: OAuthQuotaWindowKind::Time,
            limit_window_seconds: Some(18_000),
            window_reset_at: Some(1_900_003_600),
            estimated_capacity_usd: 1.0,
            estimated_used_usd: 0.11,
            estimated_remaining_usd: 0.89,
            sample_cost_usd: 0.01,
            sample_used_percent: 1.0,
            sample_started_at: 1_899_999_700,
            sample_ended_at: 1_900_000_000,
            unpriced_request_count: 3,
            pricing_basis: "openai_api_standard_2026_08_11".to_owned(),
        }],
    });
    let value = serde_json::to_value(response).expect("quota response");

    assert_eq!(value["credits"]["balance"], "17.50");
    assert_eq!(value["access"]["reached_type"], "rate_limit_reached");
    assert_eq!(value["usd_estimates"][0]["estimated_capacity_usd"], 1.0);
    assert_eq!(value["usd_estimates"][0]["unpriced_request_count"], 3);
    assert_eq!(
        value["usd_estimates"][0]["pricing_basis"],
        "openai_api_standard_2026_08_11"
    );
}

#[test]
fn serializes_grok_billing_and_subscription_without_secrets() {
    let response = OAuthQuotaResponse::from(OAuthQuotaSnapshot {
        fetched_at: 1_900_000_000,
        usage: OAuthQuotaUsage {
            rate_limit: None,
            credits: None,
            access: None,
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
        usd_estimates: Vec::new(),
    });
    let value = serde_json::to_value(response).expect("quota response");

    assert_eq!(value["subscription_tier"], "SuperGrokPro");
    assert_eq!(value["billing"]["currency"], "USD");
    assert_eq!(value["billing"]["prepaid_balance_minor"], -2500);
    assert_eq!(value["token_balance"]["source"], "upstream");
    assert_eq!(value["token_balance"]["remaining"], 0);
    assert_eq!(value["account_status"]["authentication"], "valid");
    assert_eq!(
        value["account_status"]["quota_exhaustion"]["used"],
        1_065_387
    );
}
