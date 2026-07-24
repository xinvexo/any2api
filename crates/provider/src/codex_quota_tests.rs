use http::{Method, header};

use super::*;

fn token() -> OAuthTokenMaterial {
    OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "access-secret".into(),
        Some("refresh-secret".into()),
        None,
        Some(42),
        Some("account-123".into()),
        None,
    )
    .expect("token")
}

#[test]
fn builds_fixed_query_and_reset_plans_without_debugging_secrets() {
    let (usage, credits) = query_plan(&token()).expect("query plan").into_parts();
    assert_eq!(usage.method, Method::GET);
    assert_eq!(
        usage.url.as_str(),
        "https://chatgpt.com/backend-api/wham/usage"
    );
    assert_eq!(
        credits.url.as_str(),
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits"
    );
    assert_eq!(usage.headers[header::AUTHORIZATION], "Bearer access-secret");
    assert_eq!(usage.headers["chatgpt-account-id"], "account-123");
    assert_eq!(usage.headers["openai-beta"], "codex-1");
    assert_eq!(usage.headers["originator"], "Codex Desktop");
    assert!(!format!("{usage:?}").contains("access-secret"));

    let reset = reset_plan(&token(), "00000000-0000-4000-8000-000000000001").expect("reset plan");
    assert_eq!(reset.method, Method::POST);
    assert_eq!(
        reset.url.as_str(),
        "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume"
    );
    assert_eq!(reset.headers[header::CONTENT_TYPE], "application/json");
    let body: Value = serde_json::from_slice(&reset.body).expect("reset body");
    assert_eq!(
        body["redeem_request_id"],
        "00000000-0000-4000-8000-000000000001"
    );
    assert!(!format!("{reset:?}").contains("00000000-0000"));
}

#[test]
fn quota_plan_requires_the_codex_account_id() {
    let token = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "access-secret".into(),
        None,
        None,
        None,
        None,
        None,
    )
    .expect("token");

    assert!(matches!(
        query_plan(&token),
        Err(ProviderError::InvalidCredential(_))
    ));
}

#[test]
fn parses_primary_secondary_windows_and_usage_credit_count() {
    let usage = parse_usage(
        br#"{
          "rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": {
              "used_percent": 25.5,
              "limit_window_seconds": 18000,
              "reset_after_seconds": 120,
              "reset_at": 1900000000
            },
            "secondary_window": {
              "used_percent": 80.0,
              "limit_window_seconds": 604800,
              "reset_after_seconds": 3600,
              "reset_at": 1900003600
            }
          },
          "rate_limit_reset_credits": {"available_count": 2}
        }"#,
    )
    .expect("usage");

    let limit = usage.rate_limit.expect("rate limit");
    assert!(limit.allowed);
    assert!(!limit.limit_reached);
    assert_eq!(
        limit.primary_window.expect("primary").limit_window_seconds,
        18_000
    );
    assert_eq!(
        limit.secondary_window.expect("secondary").used_percent,
        80.0
    );
    assert_eq!(
        usage.reset_credits.expect("reset credits").available_count,
        2
    );
}

#[test]
fn details_count_and_filtered_credit_records_are_sanitized() {
    let credits = parse_reset_credits(
        br#"{
          "availableCount": "2",
          "credits": [
            {"reset_type":"codex_rate_limits","status":"redeemed","expires_at":"ignored"},
            {"reset_type":"other","status":"available","expires_at":"ignored"},
            {"reset_type":"codex_rate_limits","status":"available","expires_at":"2026-07-25T00:00:00Z"},
            {"resetType":"codex_rate_limits","status":"available","expiresAt":"2026-07-26T00:00:00Z"}
          ]
        }"#,
    )
    .expect("credits")
    .expect("credit data");

    assert_eq!(credits.available_count, 2);
    assert_eq!(
        credits
            .credits
            .iter()
            .map(|credit| credit.expires_at.as_str())
            .collect::<Vec<_>>(),
        ["2026-07-25T00:00:00Z", "2026-07-26T00:00:00Z"]
    );
}

#[test]
fn credit_array_counts_available_records_even_without_expiry() {
    let credits = parse_reset_credits(
        br#"[
          {"status":"available"},
          {"status":"available","expires_at":"2026-07-25T00:00:00Z"},
          {"status":"redeemed","expires_at":"ignored"}
        ]"#,
    )
    .expect("credits")
    .expect("credit data");

    assert_eq!(credits.available_count, 2);
    assert_eq!(credits.credits.len(), 1);
}

#[test]
fn reset_response_must_confirm_a_reset_window() {
    assert_eq!(
        parse_reset_result(br#"{"code":"ok","windows_reset":2}"#)
            .expect("reset result")
            .windows_reset,
        2
    );
    assert!(matches!(
        parse_reset_result(br#"{"code":"ok","windows_reset":0}"#),
        Err(ProviderError::InvalidResponse(_))
    ));
}
