//! Claude quota request and parsing contracts.

use any2api_domain::ProviderKind;
use http::{Method, header};

use super::*;

fn token() -> OAuthTokenMaterial {
    OAuthTokenMaterial::new(
        ProviderKind::Claude,
        "access-secret".into(),
        Some("refresh-secret".into()),
        None,
        None,
        None,
        None,
    )
    .expect("token")
}

#[test]
fn builds_fixed_usage_query_with_claude_code_identity() {
    let (usage, probe, credits) = query_plan(&token()).expect("query plan").into_parts();

    assert_eq!(usage.method, Method::GET);
    assert_eq!(
        usage.url.as_str(),
        "https://api.anthropic.com/api/oauth/usage"
    );
    assert_eq!(usage.headers[header::AUTHORIZATION], "Bearer access-secret");
    assert_eq!(
        usage.headers[header::ACCEPT],
        "application/json, text/plain, */*"
    );
    assert_eq!(usage.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(usage.headers["anthropic-beta"], "oauth-2025-04-20");
    assert_eq!(usage.headers[header::USER_AGENT], "claude-code/2.1.7");
    assert!(usage.body.is_empty());
    assert!(probe.is_none());
    assert!(credits.is_none());
    assert!(!format!("{usage:?}").contains("access-secret"));
}

#[test]
fn parses_all_supported_usage_windows() {
    let usage = parse_usage(
        br#"{
          "five_hour":{"utilization":12.5,"resets_at":"2030-01-01T01:00:00Z"},
          "seven_day":{"utilization":34.0,"resets_at":"2030-01-08T00:00:00Z"},
          "seven_day_sonnet":{"utilization":56.0,"resets_at":"2030-01-08T00:00:00Z"},
          "seven_day_overage_included":{"utilization":78.0,"resets_at":"2030-01-08T00:00:00Z"},
          "unknown_secret":{"token":"must-not-escape"}
        }"#,
    )
    .expect("quota usage");

    let limit = usage.rate_limit.expect("rate limit");
    assert_eq!(limit.allowed, None);
    assert_eq!(limit.limit_reached, None);
    assert_eq!(
        limit
            .windows
            .iter()
            .map(|window| (window.id, window.used_percent, window.limit_window_seconds))
            .collect::<Vec<_>>(),
        [
            ("five_hour", 12.5, Some(18_000)),
            ("seven_day", 34.0, Some(604_800)),
            ("seven_day_sonnet", 56.0, Some(604_800)),
            ("seven_day_overage_included", 78.0, Some(604_800)),
        ]
    );
    assert_eq!(limit.windows[0].reset_at, Some(1_893_459_600));
    assert!(usage.reset_credits.is_none());
}

#[test]
fn accepts_missing_optional_windows_but_rejects_empty_or_invalid_payloads() {
    let usage =
        parse_usage(br#"{"five_hour":{"utilization":0.0,"resets_at":"2030-01-01T00:00:00Z"}}"#)
            .expect("one quota window");
    assert_eq!(usage.rate_limit.expect("rate limit").windows.len(), 1);

    for body in [
        br#"{}"#.as_slice(),
        br#"{"five_hour":{"utilization":-1,"resets_at":"2030-01-01T00:00:00Z"}}"#,
        br#"{"five_hour":{"utilization":1,"resets_at":"tomorrow"}}"#,
        br#"{"five_hour":{"utilization":"1","resets_at":"2030-01-01T00:00:00Z"}}"#,
    ] {
        assert!(parse_usage(body).is_err());
    }
}
