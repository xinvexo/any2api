//! Grok billing request and parsing contracts.

use any2api_domain::ProviderKind;
use http::{Method, header};

use super::quota::{parse_usage, query_plan};
use crate::OAuthTokenMaterial;

fn token() -> OAuthTokenMaterial {
    OAuthTokenMaterial::new(
        ProviderKind::Grok,
        "access-secret".into(),
        Some("refresh-secret".into()),
        None,
        None,
        Some("subject-1".into()),
        None,
    )
    .expect("token")
}

#[test]
fn builds_single_billing_query_with_cli_identity() {
    let (usage, credits) = query_plan(&token()).expect("query plan").into_parts();

    assert_eq!(usage.method, Method::GET);
    assert_eq!(
        usage.url.as_str(),
        "https://cli-chat-proxy.grok.com/v1/billing?format=credits"
    );
    assert_eq!(usage.headers[header::AUTHORIZATION], "Bearer access-secret");
    assert_eq!(usage.headers["x-xai-token-auth"], "xai-grok-cli");
    assert_eq!(usage.headers["x-grok-client-version"], "0.2.93");
    assert_eq!(usage.headers[header::ACCEPT], "application/json");
    assert!(usage.body.is_empty());
    assert!(credits.is_none());
    assert!(!format!("{usage:?}").contains("access-secret"));
}

#[test]
fn parses_weekly_credit_usage_without_reset_credits() {
    let usage = parse_usage(
        br#"{
          "config": {
            "currentPeriod": {
              "type": "WEEKLY",
              "start": "2030-01-01T00:00:00Z",
              "end": "2030-01-08T00:00:00Z"
            },
            "creditUsagePercent": 37.5,
            "productUsage": [{"product":"Api","usagePercent":37.5}]
          }
        }"#,
    )
    .expect("billing usage");

    let rate_limit = usage.rate_limit.expect("rate limit");
    assert!(rate_limit.allowed);
    assert!(!rate_limit.limit_reached);
    let window = rate_limit.primary_window.expect("weekly window");
    assert_eq!(window.used_percent, 37.5);
    assert_eq!(window.limit_window_seconds, 604_800);
    assert_eq!(window.reset_at, 1_894_060_800);
    assert!(rate_limit.secondary_window.is_none());
    assert!(usage.reset_credits.is_none());
}

#[test]
fn rejects_missing_negative_and_non_weekly_billing_values() {
    for body in [
        br#"{}"#.as_slice(),
        br#"{"config":{"currentPeriod":{"type":"WEEKLY","start":"2030-01-01T00:00:00Z","end":"2030-01-08T00:00:00Z"},"creditUsagePercent":-1}}"#,
        br#"{"config":{"currentPeriod":{"type":"MONTHLY","start":"2030-01-01T00:00:00Z","end":"2030-02-01T00:00:00Z"},"creditUsagePercent":1}}"#,
        br#"{"config":{"currentPeriod":{"type":"WEEKLY","start":"2030-01-08T00:00:00Z","end":"2030-01-01T00:00:00Z"},"creditUsagePercent":1}}"#,
    ] {
        assert!(parse_usage(body).is_err());
    }
}
