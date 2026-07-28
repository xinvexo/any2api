//! Grok billing and subscription request contracts.

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, StatusCode, header};

use super::{parse_subscription, parse_token_balance, parse_usage, query_plan, token_balance_plan};
use crate::{
    OAuthTokenMaterial,
    api::{
        OAuthQuotaTokenBalanceSource, OAuthQuotaUsage, OAuthQuotaWindowKind, UpstreamResponseMeta,
    },
};

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

fn free_usage() -> OAuthQuotaUsage {
    let mut usage =
        parse_usage(br#"{"config":{"isUnifiedBillingUser":true}}"#).expect("billing usage");
    usage.apply_supplement(
        parse_subscription(br#"{"subscriptionTier":null}"#).expect("free subscription"),
    );
    usage
}

#[test]
fn free_tier_builds_a_minimal_token_balance_probe() {
    let usage = free_usage();
    let plan = token_balance_plan(&token(Some("subject-1")), &usage)
        .expect("token balance plan")
        .expect("Free probe");

    assert_eq!(plan.method, Method::POST);
    assert_eq!(
        plan.url.as_str(),
        "https://cli-chat-proxy.grok.com/v1/chat/completions"
    );
    assert_eq!(plan.headers[header::AUTHORIZATION], "Bearer access-secret");
    assert_eq!(plan.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(plan.headers["x-grok-model-override"], "grok-4.5");
    assert_eq!(plan.headers["x-userid"], "subject-1");
    let body: serde_json::Value = serde_json::from_slice(&plan.body).expect("probe JSON");
    assert_eq!(body["model"], "grok-4.5");
    assert_eq!(body["max_tokens"], 1);
    assert_eq!(body["stream"], false);
    assert_eq!(body["messages"][0]["content"], "ping");

    let mut paid = usage;
    paid.apply_supplement(
        parse_subscription(br#"{"subscriptionTier":"SuperGrokPro"}"#).expect("paid subscription"),
    );
    assert!(
        token_balance_plan(&token(Some("subject-1")), &paid)
            .expect("paid balance plan")
            .is_none()
    );
}

#[test]
fn parses_the_current_free_token_limit_from_response_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-ratelimit-limit-tokens",
        HeaderValue::from_static("2000000"),
    );
    headers.insert(
        "x-ratelimit-remaining-tokens",
        HeaderValue::from_static("1750000"),
    );
    let balance = parse_token_balance(
        &free_usage(),
        &UpstreamResponseMeta {
            status: StatusCode::OK,
            headers,
        },
        br#"{"choices":[]}"#,
    )
    .expect("token headers")
    .expect("token balance");

    assert_eq!(balance.source, OAuthQuotaTokenBalanceSource::Upstream);
    assert_eq!(balance.used, 250_000);
    assert_eq!(balance.limit, 2_000_000);
    assert_eq!(balance.remaining, 1_750_000);
    assert_eq!(balance.window_seconds, None);
}

#[test]
fn incomplete_or_invalid_token_headers_remain_unknown() {
    for (limit, remaining) in [
        (Some("2000000"), None),
        (None, Some("1750000")),
        (Some("invalid"), Some("1750000")),
        (Some("2000000"), Some("2000001")),
        (Some("0"), Some("0")),
        (Some("9007199254740992"), Some("1")),
    ] {
        let mut headers = HeaderMap::new();
        if let Some(limit) = limit {
            headers.insert(
                "x-ratelimit-limit-tokens",
                HeaderValue::from_str(limit).expect("limit header"),
            );
        }
        if let Some(remaining) = remaining {
            headers.insert(
                "x-ratelimit-remaining-tokens",
                HeaderValue::from_str(remaining).expect("remaining header"),
            );
        }
        assert!(
            parse_token_balance(
                &free_usage(),
                &UpstreamResponseMeta {
                    status: StatusCode::OK,
                    headers,
                },
                b"{}",
            )
            .expect("invalid headers stay unknown")
            .is_none()
        );
    }
}

#[test]
fn parses_actual_limit_from_a_free_exhaustion_response() {
    let balance = parse_token_balance(
        &free_usage(),
        &UpstreamResponseMeta {
            status: StatusCode::TOO_MANY_REQUESTS,
            headers: HeaderMap::new(),
        },
        br#"{"error":{"code":"subscription:free-usage-exhausted","message":"tokens (actual/limit): 2042591/2000000; Usage resets over a rolling window"}}"#,
    )
    .expect("exhaustion response")
    .expect("exhaustion balance");

    assert_eq!(balance.source, OAuthQuotaTokenBalanceSource::Upstream);
    assert_eq!(balance.used, 2_042_591);
    assert_eq!(balance.limit, 2_000_000);
    assert_eq!(balance.remaining, 0);
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
