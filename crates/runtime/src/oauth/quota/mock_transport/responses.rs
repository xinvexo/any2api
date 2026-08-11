//! Provider response fixtures emitted by the OAuth quota mock transport.

use std::sync::atomic::Ordering;

use bytes::Bytes;
use http::{HeaderMap, StatusCode};

use super::{AuthenticationMode, QuotaTransport};

pub(super) type MockResponse = (StatusCode, HeaderMap, Bytes);

impl QuotaTransport {
    pub(super) fn refresh_response(&self) -> MockResponse {
        self.refresh_calls.fetch_add(1, Ordering::AcqRel);
        if self.authentication == AuthenticationMode::RefreshRejected {
            return service_unavailable();
        }
        if self.authentication == AuthenticationMode::RefreshInvalidGrant {
            return invalid_grant();
        }
        if self.authentication == AuthenticationMode::RefreshTokenReused {
            return refresh_token_reused();
        }
        ok(Bytes::from_static(
            br#"{"access_token":"new-access","expires_in":3600}"#,
        ))
    }

    pub(super) fn codex_usage_response(&self) -> MockResponse {
        match self.authentication {
            AuthenticationMode::CodexCloudflareBlocked => {
                self.usage_calls.fetch_add(1, Ordering::AcqRel);
                return cloudflare_blocked();
            }
            AuthenticationMode::CodexUnknownForbidden => {
                self.usage_calls.fetch_add(1, Ordering::AcqRel);
                return forbidden_unknown();
            }
            _ => {}
        }
        if let Some(rejected) = self.authentication_rejection() {
            return rejected;
        }
        if self.codex_exhausted.load(Ordering::Acquire) {
            return codex_exhausted(self.codex_has_credits.load(Ordering::Acquire));
        }
        ok(Bytes::from_static(
            br#"{"rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":25.0,"limit_window_seconds":18000,"reset_after_seconds":60,"reset_at":1900000000},"secondary_window":null},"rate_limit_reset_credits":{"available_count":7}}"#,
        ))
    }

    pub(super) fn claude_usage_response(&self) -> MockResponse {
        if let Some(rejected) = self.authentication_rejection() {
            return rejected;
        }
        ok(Bytes::from_static(
            br#"{"five_hour":{"utilization":12.5,"resets_at":"2030-01-01T01:00:00Z"},"seven_day":{"utilization":34.0,"resets_at":"2030-01-08T00:00:00Z"},"seven_day_sonnet":{"utilization":56.0,"resets_at":"2030-01-08T00:00:00Z"},"seven_day_overage_included":{"utilization":78.0,"resets_at":"2030-01-08T00:00:00Z"}}"#,
        ))
    }

    pub(super) fn grok_billing_response(&self) -> MockResponse {
        if let Some(rejected) = self.authentication_rejection() {
            return rejected;
        }
        let body = if self.grok_unified {
            Bytes::from_static(
                br#"{"config":{"currentPeriod":{"type":"USAGE_PERIOD_TYPE_WEEKLY","start":"2030-01-01T00:00:00Z","end":"2030-01-08T00:00:00Z"},"onDemandCap":{"val":"0"},"onDemandUsed":{"val":"0"},"isUnifiedBillingUser":true,"prepaidBalance":{"val":"2500"}}}"#,
            )
        } else {
            Bytes::from_static(
                br#"{"config":{"currentPeriod":{"type":"WEEKLY","start":"2030-01-01T00:00:00Z","end":"2030-01-08T00:00:00Z"},"creditUsagePercent":37.5}}"#,
            )
        };
        ok(body)
    }

    pub(super) fn grok_subscription_response(&self) -> MockResponse {
        if self.grok_unified {
            ok(Bytes::from_static(
                br#"{"subscriptionTier":null,"teamBlockedReasons":[]}"#,
            ))
        } else {
            ok(Bytes::from_static(
                br#"{"subscriptionTier":"SuperGrokPro","userBlockedReason":"BLOCKED_REASON_BILLING","teamBlockedReasons":["BLOCKED_REASON_NO_LOGS"]}"#,
            ))
        }
    }

    pub(super) fn reset_credits_response(&self) -> MockResponse {
        ok(Bytes::from(
            serde_json::json!({
                "available_count": self.available_count.load(Ordering::Acquire),
                "credits": [{
                    "reset_type": "codex_rate_limits",
                    "status": "available",
                    "expires_at": "2026-07-25T00:00:00Z"
                }]
            })
            .to_string(),
        ))
    }

    pub(super) async fn consume_response(&self) -> MockResponse {
        if self.block_consume {
            self.consume_started.add_permits(1);
            self.consume_release
                .acquire()
                .await
                .expect("consume release signal")
                .forget();
        }
        self.consume_calls.fetch_add(1, Ordering::AcqRel);
        self.available_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_sub(1)
            })
            .expect("consume requires an available credit");
        ok(Bytes::from_static(br#"{"code":"ok","windows_reset":2}"#))
    }

    fn authentication_rejection(&self) -> Option<MockResponse> {
        let attempt = self.usage_calls.fetch_add(1, Ordering::AcqRel);
        match self.authentication {
            AuthenticationMode::Accepted => None,
            AuthenticationMode::RejectOnce if attempt == 0 => Some(unauthorized()),
            AuthenticationMode::RejectOnce => None,
            AuthenticationMode::AlwaysReject => Some(unauthorized()),
            AuthenticationMode::AlwaysForbidden => Some(forbidden()),
            AuthenticationMode::RefreshRejected
            | AuthenticationMode::RefreshInvalidGrant
            | AuthenticationMode::RefreshTokenReused => Some(unauthorized()),
            AuthenticationMode::CodexCloudflareBlocked
            | AuthenticationMode::CodexUnknownForbidden => None,
        }
    }
}

fn codex_exhausted(has_credits: bool) -> MockResponse {
    if has_credits {
        return ok(Bytes::from_static(
            br#"{"rate_limit":{"allowed":false,"limit_reached":true,"primary_window":{"used_percent":100.0,"limit_window_seconds":18000,"reset_after_seconds":60,"reset_at":1900000000},"secondary_window":null},"credits":{"has_credits":true,"unlimited":false,"balance":"17.50"},"spend_control":{"reached":false},"rate_limit_reached_type":{"type":"rate_limit_reached"},"rate_limit_reset_credits":{"available_count":7}}"#,
        ));
    }
    ok(Bytes::from_static(
        br#"{"rate_limit":{"allowed":false,"limit_reached":true,"primary_window":{"used_percent":100.0,"limit_window_seconds":18000,"reset_after_seconds":60,"reset_at":1900000000},"secondary_window":null},"rate_limit_reset_credits":{"available_count":7}}"#,
    ))
}

fn ok(body: Bytes) -> MockResponse {
    (StatusCode::OK, HeaderMap::new(), body)
}

fn unauthorized() -> MockResponse {
    (
        StatusCode::UNAUTHORIZED,
        HeaderMap::new(),
        Bytes::from_static(b"{}"),
    )
}

fn forbidden() -> MockResponse {
    (
        StatusCode::FORBIDDEN,
        HeaderMap::new(),
        Bytes::from_static(br#"{"code":"unauthorized:blocked-user"}"#),
    )
}

fn forbidden_unknown() -> MockResponse {
    (
        StatusCode::FORBIDDEN,
        HeaderMap::new(),
        Bytes::from_static(br#"{"error":{"code":"unknown_forbidden"}}"#),
    )
}

fn cloudflare_blocked() -> MockResponse {
    (
        StatusCode::FORBIDDEN,
        HeaderMap::new(),
        Bytes::from_static(
            b"<html><body>Cloudflare error: Sorry, you have been blocked</body></html>",
        ),
    )
}

fn service_unavailable() -> MockResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        HeaderMap::new(),
        Bytes::from_static(b"{}"),
    )
}

fn invalid_grant() -> MockResponse {
    (
        StatusCode::BAD_REQUEST,
        HeaderMap::new(),
        Bytes::from_static(br#"{"error":"invalid_grant"}"#),
    )
}

fn refresh_token_reused() -> MockResponse {
    (
        StatusCode::UNAUTHORIZED,
        HeaderMap::new(),
        Bytes::from_static(br#"{"error":{"code":"refresh_token_reused"}}"#),
    )
}
