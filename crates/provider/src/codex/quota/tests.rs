//! Codex OAuth quota parsing and request contracts.

use any2api_domain::ProviderKind;
use http::{HeaderMap, Method, StatusCode, header};
use serde_json::Value;

use super::*;
use crate::api::{
    OAuthQuotaReachedType, OAuthQuotaRejection, OAuthTokenMaterial, ProviderError,
    UpstreamResponseMeta,
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
    let (usage, supplement, credits) = query_plan(&token()).expect("query plan").into_parts();
    let credits = credits.expect("Codex reset credit plan");
    assert!(supplement.is_none());
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
    assert_eq!(
        classify_quota_rejection(
            &forbidden,
            b"<html><body>Cloudflare error: Sorry, you have been blocked</body></html>",
        ),
        OAuthQuotaRejection::ProviderEgressRestricted
    );
    for body in [
        br#"{"error":{"message":"account_deactivated"}}"#.as_slice(),
        br#"{"metadata":{"code":"account_suspended"}}"#.as_slice(),
        br#"{"error":{"code":"unknown_forbidden"}}"#.as_slice(),
        br#"{"code":"account_disabled","error":{"code":"unsupported_country_region_territory"}}"#
            .as_slice(),
        b"<html>Cloudflare edge response without the second marker</html>".as_slice(),
        b"<html>request blocked without the provider marker</html>".as_slice(),
        b"not-json".as_slice(),
    ] {
        assert_eq!(
            classify_quota_rejection(&forbidden, body),
            OAuthQuotaRejection::Unclassified
        );
    }
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
          "credits": {
            "has_credits": true,
            "unlimited": false,
            "balance": " 17.50 "
          },
          "spend_control": {"reached": false},
          "rate_limit_reached_type": {"type":"rate_limit_reached"},
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
    let credits = usage.credits.expect("Credits");
    assert!(credits.has_credits);
    assert!(!credits.unlimited);
    assert_eq!(credits.balance.as_deref(), Some("17.50"));
    let access = usage.access.expect("access status");
    assert_eq!(access.spend_control_reached, Some(false));
    assert_eq!(
        access.reached_type,
        Some(OAuthQuotaReachedType::RateLimitReached)
    );
    assert_eq!(
        usage.reset_credits.expect("reset credits").available_count,
        2
    );
}

#[test]
fn parses_unlimited_credits_and_workspace_hard_stops() {
    let usage = parse_usage(
        br#"{
          "rate_limit": {"allowed":false,"limit_reached":true},
          "credits": {"has_credits":false,"unlimited":true,"balance":null},
          "spend_control": {"reached":true},
          "rate_limit_reached_type": {"type":"workspace_member_usage_limit_reached"}
        }"#,
    )
    .expect("usage");

    assert!(usage.credits.expect("Credits").usable());
    let access = usage.access.expect("access status");
    assert!(access.workspace_hard_stop());
    assert_eq!(
        access.reached_type,
        Some(OAuthQuotaReachedType::WorkspaceMemberUsageLimitReached)
    );
}

#[test]
fn preserves_zero_and_hidden_real_credit_balances() {
    let zero = parse_usage(br#"{"credits":{"has_credits":true,"unlimited":false,"balance":"0"}}"#)
        .expect("zero Credits")
        .credits
        .expect("Credits");
    let hidden =
        parse_usage(br#"{"credits":{"has_credits":true,"unlimited":false,"balance":null}}"#)
            .expect("hidden Credits")
            .credits
            .expect("Credits");

    assert_eq!(zero.balance.as_deref(), Some("0"));
    assert!(zero.usable());
    assert_eq!(hidden.balance, None);
    assert!(hidden.usable());
}

#[test]
fn maps_every_declared_quota_reached_type() {
    let cases = [
        (
            "rate_limit_reached",
            OAuthQuotaReachedType::RateLimitReached,
        ),
        (
            "workspace_owner_credits_depleted",
            OAuthQuotaReachedType::WorkspaceOwnerCreditsDepleted,
        ),
        (
            "workspace_member_credits_depleted",
            OAuthQuotaReachedType::WorkspaceMemberCreditsDepleted,
        ),
        (
            "workspace_owner_usage_limit_reached",
            OAuthQuotaReachedType::WorkspaceOwnerUsageLimitReached,
        ),
        (
            "workspace_member_usage_limit_reached",
            OAuthQuotaReachedType::WorkspaceMemberUsageLimitReached,
        ),
    ];
    for (wire, expected) in cases {
        let body = serde_json::to_vec(&serde_json::json!({
            "rate_limit_reached_type": {"type": wire},
        }))
        .expect("quota fixture");
        let access = parse_usage(&body)
            .expect("declared reached type")
            .access
            .expect("access status");
        assert_eq!(access.reached_type, Some(expected));
    }
}

#[test]
fn rejects_non_decimal_credit_balances_and_ignores_unknown_reached_types() {
    assert!(matches!(
        parse_usage(br#"{"credits":{"has_credits":true,"unlimited":false,"balance":"$17"}}"#,),
        Err(ProviderError::InvalidResponse(_))
    ));

    let usage = parse_usage(br#"{"rate_limit_reached_type":{"type":"future_limit_type"}}"#)
        .expect("unknown reached type remains neutral");
    assert!(usage.access.is_none());
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
