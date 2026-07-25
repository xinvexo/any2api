//! Grok billing request and parsing contracts.

use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use serde_json::Value;

use super::quota::{parse_probe, parse_usage, query_plan};
use crate::{
    OAuthTokenMaterial,
    api::{OAuthQuotaUsageParse, OAuthQuotaWindowKind, UpstreamResponseMeta},
};

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
    let (usage, probe, credits) = query_plan(&token()).expect("query plan").into_parts();
    let probe = probe.expect("usage probe");

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
    assert_eq!(probe.method, Method::POST);
    assert_eq!(
        probe.url.as_str(),
        "https://cli-chat-proxy.grok.com/v1/responses"
    );
    assert_eq!(probe.headers[header::AUTHORIZATION], "Bearer access-secret");
    assert_eq!(probe.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(
        probe.headers[header::ACCEPT],
        "application/json, text/event-stream"
    );
    let body: Value = serde_json::from_slice(&probe.body).expect("probe body");
    assert_eq!(
        body,
        serde_json::json!({"model":"grok-4.5","input":"hi","stream":true})
    );
    assert!(credits.is_none());
    assert!(!format!("{usage:?}").contains("access-secret"));
    assert!(!format!("{probe:?}").contains("access-secret"));
    assert!(!format!("{probe:?}").contains("hi"));
}

#[test]
fn parses_weekly_credit_usage_without_reset_credits() {
    let usage = complete(
        parse_usage(
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
        .expect("billing usage"),
    );

    let rate_limit = usage.rate_limit.expect("rate limit");
    assert_eq!(rate_limit.allowed, Some(true));
    assert_eq!(rate_limit.limit_reached, Some(false));
    let window = &rate_limit.windows[0];
    assert_eq!(window.id, "weekly_credits");
    assert_eq!(window.kind, OAuthQuotaWindowKind::Credits);
    assert_eq!(window.used_percent, 37.5);
    assert_eq!(window.limit_window_seconds, Some(604_800));
    assert_eq!(window.reset_at, Some(1_894_060_800));
    assert_eq!(rate_limit.windows.len(), 1);
    assert!(usage.reset_credits.is_none());
}

#[test]
fn unified_billing_without_percentage_requires_a_header_probe() {
    let parsed = parse_usage(
        br#"{
          "config": {
            "currentPeriod": {
              "type": "USAGE_PERIOD_TYPE_WEEKLY",
              "start": "2030-01-01T00:00:00Z",
              "end": "2030-01-08T00:00:00Z"
            },
            "onDemandCap": {"val":"0"},
            "onDemandUsed": {"val":"0"},
            "prepaidBalance": {"val":"0"},
            "isUnifiedBillingUser": true,
            "billingPeriodStart": "2030-01-01T00:00:00Z",
            "billingPeriodEnd": "2030-01-08T00:00:00Z"
          }
        }"#,
    )
    .expect("unified billing");

    assert_eq!(parsed, OAuthQuotaUsageParse::ProbeRequired);
}

#[test]
fn parses_request_and_token_quota_headers() {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("x-ratelimit-limit-requests", "100"),
        ("x-ratelimit-remaining-requests", "75"),
        ("x-ratelimit-reset-requests", "1894060800"),
        ("x-ratelimit-limit-tokens", "1000"),
        ("x-ratelimit-remaining-tokens", "400"),
        ("x-ratelimit-reset-tokens", "1894060800000"),
    ] {
        headers.insert(
            http::header::HeaderName::from_bytes(name.as_bytes()).expect("header name"),
            HeaderValue::from_static(value),
        );
    }
    let usage = parse_probe(&UpstreamResponseMeta {
        status: StatusCode::OK,
        headers,
    })
    .expect("quota headers");

    let rate_limit = usage.rate_limit.expect("rate limit");
    assert_eq!(rate_limit.allowed, Some(true));
    let requests = &rate_limit.windows[0];
    assert_eq!(requests.id, "requests");
    assert_eq!(requests.kind, OAuthQuotaWindowKind::Requests);
    assert_eq!(requests.used_percent, 25.0);
    assert_eq!(requests.limit_window_seconds, None);
    assert_eq!(requests.reset_at, Some(1_894_060_800));
    let tokens = &rate_limit.windows[1];
    assert_eq!(tokens.id, "tokens");
    assert_eq!(tokens.kind, OAuthQuotaWindowKind::Tokens);
    assert_eq!(tokens.used_percent, 60.0);
    assert_eq!(tokens.reset_at, Some(1_894_060_800));
}

#[test]
fn accepts_headerless_429_as_an_explicit_exhausted_observation() {
    let usage = parse_probe(&UpstreamResponseMeta {
        status: StatusCode::TOO_MANY_REQUESTS,
        headers: HeaderMap::new(),
    })
    .expect("429 observation");
    let rate_limit = usage.rate_limit.expect("rate limit");
    assert_eq!(rate_limit.allowed, Some(false));
    assert_eq!(rate_limit.limit_reached, Some(true));
    assert!(rate_limit.windows.is_empty());
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

#[test]
fn rejects_incomplete_or_inconsistent_probe_headers() {
    for headers in [
        headers(&[("x-ratelimit-limit-requests", "100")]),
        headers(&[
            ("x-ratelimit-limit-requests", "100"),
            ("x-ratelimit-remaining-requests", "101"),
        ]),
        headers(&[
            ("x-ratelimit-limit-tokens", "100"),
            ("x-ratelimit-remaining-tokens", "50"),
            ("x-ratelimit-reset-tokens", "tomorrow"),
        ]),
    ] {
        assert!(
            parse_probe(&UpstreamResponseMeta {
                status: StatusCode::OK,
                headers,
            })
            .is_err()
        );
    }
}

fn complete(parsed: OAuthQuotaUsageParse) -> crate::api::OAuthQuotaUsage {
    match parsed {
        OAuthQuotaUsageParse::Complete(usage) => usage,
        OAuthQuotaUsageParse::ProbeRequired => panic!("unexpected probe requirement"),
    }
}

fn headers(values: &[(&str, &'static str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in values {
        headers.insert(
            http::header::HeaderName::from_bytes(name.as_bytes()).expect("header name"),
            HeaderValue::from_static(value),
        );
    }
    headers
}
