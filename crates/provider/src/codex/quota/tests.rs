//! Codex OAuth quota parsing and request contracts.

use any2api_domain::ProviderKind;
use http::{HeaderMap, Method, StatusCode, header};
use serde_json::Value;

use super::*;
use crate::{
    OAuthProviderEgressStatus, OAuthQuotaRejection, OAuthTokenMaterial, ProviderError,
    api::UpstreamResponseMeta,
};

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
    let (usage, probe, credits) = query_plan(&token()).expect("query plan").into_parts();
    let credits = credits.expect("Codex reset credit plan");
    assert!(probe.is_none());
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
fn builds_account_free_egress_probe() {
    let probe = egress_probe_plan().expect("egress probe");

    assert_eq!(probe.method, Method::GET);
    assert_eq!(
        probe.url.as_str(),
        "https://chatgpt.com/backend-api/wham/usage"
    );
    assert_eq!(probe.headers[header::ACCEPT], "application/json");
    assert!(!probe.headers.contains_key(header::AUTHORIZATION));
    assert!(!probe.headers.contains_key("chatgpt-account-id"));
    assert!(probe.body.is_empty());
}

#[test]
fn quota_rejection_requires_declared_account_or_egress_codes() {
    let forbidden = UpstreamResponseMeta {
        status: StatusCode::FORBIDDEN,
        headers: HeaderMap::new(),
    };

    assert_eq!(
        classify_quota_rejection(
            &forbidden,
            br#"{"error":{"code":"unsupported_country_region_territory"}}"#,
        ),
        OAuthQuotaRejection::ProviderEgressRestricted
    );
    assert_eq!(
        classify_quota_rejection(
            &forbidden,
            br#"{"code":"account_deactivated","error":{"message":"ignored"}}"#,
        ),
        OAuthQuotaRejection::AccountRestricted
    );
    for body in [
        br#"{"error":{"message":"account_deactivated"}}"#.as_slice(),
        br#"{"metadata":{"code":"account_suspended"}}"#.as_slice(),
        br#"{"error":{"code":"unknown_forbidden"}}"#.as_slice(),
        br#"{"code":"account_disabled","error":{"code":"unsupported_country_region_territory"}}"#
            .as_slice(),
        b"not-json".as_slice(),
    ] {
        assert_eq!(
            classify_quota_rejection(&forbidden, body),
            OAuthQuotaRejection::Unclassified
        );
    }
}

#[test]
fn account_free_probe_distinguishes_reachable_and_rejected_egress() {
    let status = |status| UpstreamResponseMeta {
        status,
        headers: HeaderMap::new(),
    };

    assert_eq!(
        classify_egress(&status(StatusCode::UNAUTHORIZED), b"{}"),
        OAuthProviderEgressStatus::Reachable
    );
    assert_eq!(
        classify_egress(&status(StatusCode::FORBIDDEN), b"{}"),
        OAuthProviderEgressStatus::Restricted
    );
    assert_eq!(
        classify_egress(&status(StatusCode::SERVICE_UNAVAILABLE), b"{}"),
        OAuthProviderEgressStatus::Unverified
    );
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
    assert_eq!(limit.allowed, Some(true));
    assert_eq!(limit.limit_reached, Some(false));
    assert_eq!(limit.windows[0].limit_window_seconds, Some(18_000));
    assert_eq!(limit.windows[0].id, "primary");
    assert_eq!(limit.windows[1].id, "secondary");
    assert_eq!(limit.windows[1].used_percent, 80.0);
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
