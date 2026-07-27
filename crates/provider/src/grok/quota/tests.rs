//! Grok billing and subscription request contracts.

use any2api_domain::ProviderKind;
use http::{Method, header};

use super::{local_token_quota_policy, parse_subscription, parse_usage, query_plan};
use crate::{OAuthTokenMaterial, api::OAuthQuotaWindowKind};

fn token(account_id: Option<&str>) -> OAuthTokenMaterial {
    OAuthTokenMaterial::new(
        ProviderKind::Grok,
        "access-secret".into(),
        Some("refresh-secret".into()),
        None,
        None,
        account_id.map(str::to_owned),
        None,
    )
    .expect("token")
}

#[test]
fn builds_billing_and_subscription_queries_with_official_identity() {
    let (usage, supplement, credits) = query_plan(&token(Some("subject-1")))
        .expect("query plan")
        .into_parts();
    let supplement = supplement.expect("subscription query");

    assert_eq!(usage.method, Method::GET);
    assert_eq!(
        usage.url.as_str(),
        "https://cli-chat-proxy.grok.com/v1/billing?format=credits"
    );
    assert_eq!(
        supplement.url.as_str(),
        "https://cli-chat-proxy.grok.com/v1/user?include=subscription"
    );
    for request in [&usage, &supplement] {
        assert_eq!(
            request.headers[header::AUTHORIZATION],
            "Bearer access-secret"
        );
        assert_eq!(request.headers["x-xai-token-auth"], "xai-grok-cli");
        assert_eq!(request.headers["x-userid"], "subject-1");
        assert_eq!(request.headers["x-grok-client-version"], "0.2.112");
        assert_eq!(request.headers["x-grok-client-mode"], "interactive");
        assert_eq!(request.headers[header::ACCEPT], "application/json");
        assert!(request.body.is_empty());
        assert!(!format!("{request:?}").contains("access-secret"));
    }
    assert!(credits.is_none());
}

#[test]
fn quota_query_requires_the_grok_subject() {
    assert!(query_plan(&token(None)).is_err());
}

#[test]
fn parses_weekly_credit_usage_and_billing_amounts() {
    let usage = parse_usage(
        br#"{
          "config": {
            "currentPeriod": {
              "type": "USAGE_PERIOD_TYPE_WEEKLY",
              "start": "2030-01-01T00:00:00Z",
              "end": "2030-01-08T00:00:00Z"
            },
            "creditUsagePercent": 37.5,
            "onDemandCap": {"val":"5000"},
            "onDemandUsed": {"val":"1250"},
            "prepaidBalance": {"val":"-975"},
            "isUnifiedBillingUser": true
          },
          "subscriptionTier": "SuperGrok Heavy"
        }"#,
    )
    .expect("billing usage");

    let rate_limit = usage.rate_limit.expect("rate limit");
    assert_eq!(rate_limit.allowed, Some(true));
    let window = &rate_limit.windows[0];
    assert_eq!(window.id, "weekly_credits");
    assert_eq!(window.kind, OAuthQuotaWindowKind::Credits);
    assert_eq!(window.used_percent, 37.5);
    assert_eq!(window.limit_window_seconds, Some(604_800));
    assert_eq!(window.reset_at, Some(1_894_060_800));
    let billing = usage.billing.expect("billing amounts");
    assert_eq!(billing.currency, "USD");
    assert_eq!(billing.prepaid_balance_minor, Some(-975));
    assert_eq!(billing.on_demand_used_minor, Some(1250));
    assert_eq!(billing.on_demand_cap_minor, Some(5000));
    assert_eq!(billing.is_unified_billing_user, Some(true));
    assert_eq!(usage.subscription_tier.as_deref(), Some("SuperGrok Heavy"));
}

#[test]
fn derives_legacy_monthly_usage() {
    let usage = parse_usage(
        br#"{
          "config": {
            "monthlyLimit": {"val": 2000},
            "used": {"val": "500"},
            "billingPeriodStart": "2030-01-01T00:00:00Z",
            "billingPeriodEnd": "2030-02-01T00:00:00Z"
          }
        }"#,
    )
    .expect("legacy billing");
    let window = &usage.rate_limit.expect("rate limit").windows[0];
    assert_eq!(window.id, "monthly_credits");
    assert_eq!(window.used_percent, 25.0);
    assert_eq!(window.limit_window_seconds, Some(2_678_400));
}

#[test]
fn free_billing_keeps_missing_usage_unknown() {
    let usage = parse_usage(
        br#"{
          "config": {
            "currentPeriod": {
              "type": "USAGE_PERIOD_TYPE_WEEKLY",
              "start": "2030-01-01T00:00:00Z",
              "end": "2030-01-08T00:00:00Z"
            },
            "onDemandCap": {},
            "onDemandUsed": {"val":"0"},
            "prepaidBalance": {"val":"2500"},
            "isUnifiedBillingUser": true
          }
        }"#,
    )
    .expect("unified billing");

    assert!(usage.rate_limit.is_none());
    let billing = usage.billing.expect("billing amounts");
    assert_eq!(billing.prepaid_balance_minor, Some(2500));
    assert_eq!(billing.on_demand_cap_minor, Some(0));
    assert_eq!(billing.on_demand_used_minor, Some(0));
}

#[test]
fn preserves_an_explicit_zero_usage_percent() {
    let usage = parse_usage(
        br#"{
          "config": {
            "currentPeriod": {
              "type": "USAGE_PERIOD_TYPE_WEEKLY",
              "start": "2030-01-01T00:00:00Z",
              "end": "2030-01-08T00:00:00Z"
            },
            "creditUsagePercent": 0
          }
        }"#,
    )
    .expect("explicit zero usage");

    let window = &usage.rate_limit.expect("rate limit").windows[0];
    assert_eq!(window.id, "weekly_credits");
    assert_eq!(window.used_percent, 0.0);
}

#[test]
fn parses_the_live_subscription_tier() {
    let paid = parse_subscription(
        br#"{
          "subscriptionTier":"SuperGrokPro",
          "userBlockedReason":"BLOCKED_REASON_BILLING",
          "teamBlockedReasons":["BLOCKED_REASON_NO_LOGS", "  "]
        }"#,
    )
    .expect("subscription");
    assert_eq!(paid.subscription_tier.as_deref(), Some("SuperGrokPro"));
    assert_eq!(
        paid.user_blocked_reason.as_deref(),
        Some("BLOCKED_REASON_BILLING")
    );
    assert_eq!(paid.team_blocked_reasons, ["BLOCKED_REASON_NO_LOGS"]);

    let free = parse_subscription(br#"{"subscriptionTier":null}"#).expect("free subscription");
    assert_eq!(free.subscription_tier.as_deref(), Some("Free"));
    assert!(free.user_blocked_reason.is_none());
    assert!(free.team_blocked_reasons.is_empty());
}

#[test]
fn free_tier_uses_the_default_local_one_million_token_window() {
    let mut usage =
        parse_usage(br#"{"config":{"isUnifiedBillingUser":true}}"#).expect("billing usage");
    usage.apply_supplement(
        parse_subscription(br#"{"subscriptionTier":null}"#).expect("free subscription"),
    );

    let policy = local_token_quota_policy(&usage).expect("local Free policy");
    assert_eq!(policy.limit, 1_000_000);
    assert_eq!(policy.window_seconds, 86_400);

    usage.apply_supplement(
        parse_subscription(br#"{"subscriptionTier":"SuperGrokPro"}"#).expect("paid subscription"),
    );
    assert!(local_token_quota_policy(&usage).is_none());
}

#[test]
fn rejects_malformed_billing_values() {
    for body in [
        br#"{}"#.as_slice(),
        br#"{"config":{}}"#,
        br#"{"config":{"currentPeriod":{"type":"DAILY","start":"2030-01-01T00:00:00Z","end":"2030-01-02T00:00:00Z"},"creditUsagePercent":1}}"#,
        br#"{"config":{"currentPeriod":{"type":"WEEKLY","start":"2030-01-08T00:00:00Z","end":"2030-01-01T00:00:00Z"},"creditUsagePercent":1}}"#,
        br#"{"config":{"currentPeriod":{"type":"WEEKLY","start":"2030-01-01T00:00:00Z","end":"2030-01-08T00:00:00Z"},"creditUsagePercent":-1}}"#,
        br#"{"config":{"monthlyLimit":{"val":"-1"},"used":{"val":"0"}}}"#,
        br#"{"config":{"prepaidBalance":{"val":"9007199254740992"}}}"#,
    ] {
        assert!(parse_usage(body).is_err(), "body should be rejected");
    }
}
