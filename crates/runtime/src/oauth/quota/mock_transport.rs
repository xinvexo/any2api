//! Test transport for OAuth quota and refresh behavior.

use std::sync::{
    Mutex,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};

use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum AuthenticationMode {
    Accepted,
    RejectOnce,
    AlwaysReject,
    AlwaysForbidden,
    RefreshRejected,
}

pub(super) struct CapturedQuotaRequest {
    pub(super) path: String,
    pub(super) authorization: Option<String>,
    pub(super) account_id: Option<String>,
    pub(super) grok_token_auth: Option<String>,
    pub(super) grok_client_version: Option<String>,
    pub(super) grok_user_id: Option<String>,
    pub(super) grok_client_mode: Option<String>,
    pub(super) anthropic_beta: Option<String>,
    pub(super) user_agent: Option<String>,
    pub(super) proxy_id: any2api_domain::ProxyProfileId,
    pub(super) strict_ssrf: bool,
    pub(super) body: Bytes,
}

pub(super) struct QuotaTransport {
    available_count: AtomicU32,
    authentication: AuthenticationMode,
    usage_calls: AtomicUsize,
    refresh_calls: AtomicUsize,
    consume_calls: AtomicUsize,
    block_consume: bool,
    grok_unified: bool,
    consume_started: Semaphore,
    consume_release: Semaphore,
    captured: Mutex<Vec<CapturedQuotaRequest>>,
}

impl QuotaTransport {
    pub(super) fn new(
        available_count: u32,
        authentication: AuthenticationMode,
        block_consume: bool,
    ) -> Self {
        Self {
            available_count: AtomicU32::new(available_count),
            authentication,
            usage_calls: AtomicUsize::new(0),
            refresh_calls: AtomicUsize::new(0),
            consume_calls: AtomicUsize::new(0),
            block_consume,
            grok_unified: false,
            consume_started: Semaphore::new(0),
            consume_release: Semaphore::new(0),
            captured: Mutex::new(Vec::new()),
        }
    }

    pub(super) fn new_grok_unified() -> Self {
        let mut transport = Self::new(0, AuthenticationMode::Accepted, false);
        transport.grok_unified = true;
        transport
    }

    pub(super) fn captured(&self) -> Vec<CapturedQuotaRequest> {
        self.captured
            .lock()
            .expect("captured request lock")
            .iter()
            .map(|request| CapturedQuotaRequest {
                path: request.path.clone(),
                authorization: request.authorization.clone(),
                account_id: request.account_id.clone(),
                grok_token_auth: request.grok_token_auth.clone(),
                grok_client_version: request.grok_client_version.clone(),
                grok_user_id: request.grok_user_id.clone(),
                grok_client_mode: request.grok_client_mode.clone(),
                anthropic_beta: request.anthropic_beta.clone(),
                user_agent: request.user_agent.clone(),
                proxy_id: request.proxy_id,
                strict_ssrf: request.strict_ssrf,
                body: request.body.clone(),
            })
            .collect()
    }

    pub(super) fn refresh_calls(&self) -> usize {
        self.refresh_calls.load(Ordering::Acquire)
    }

    pub(super) fn consume_calls(&self) -> usize {
        self.consume_calls.load(Ordering::Acquire)
    }

    pub(super) fn usage_calls(&self) -> usize {
        self.usage_calls.load(Ordering::Acquire)
    }

    pub(super) async fn wait_for_consume(&self) {
        self.consume_started
            .acquire()
            .await
            .expect("consume start signal")
            .forget();
    }

    pub(super) fn release_consume(&self) {
        self.consume_release.add_permits(1);
    }

    pub(super) fn usage_authorizations(&self) -> Vec<String> {
        self.captured()
            .into_iter()
            .filter(|request| request.path.ends_with("/usage"))
            .filter_map(|request| request.authorization)
            .collect()
    }
}

#[async_trait]
impl TransportManager for QuotaTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        let path = request.uri.path().to_owned();
        let captured_path = request
            .uri
            .path_and_query()
            .map_or_else(|| path.clone(), ToString::to_string);
        let authorization = request
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        self.captured
            .lock()
            .expect("captured request lock")
            .push(CapturedQuotaRequest {
                path: captured_path,
                authorization,
                account_id: header(&request, "chatgpt-account-id"),
                grok_token_auth: header(&request, "x-xai-token-auth"),
                grok_client_version: header(&request, "x-grok-client-version"),
                grok_user_id: header(&request, "x-userid"),
                grok_client_mode: header(&request, "x-grok-client-mode"),
                anthropic_beta: header(&request, "anthropic-beta"),
                user_agent: header(&request, "user-agent"),
                proxy_id: proxy.profile().id(),
                strict_ssrf: request.network_policy.strict_ssrf(),
                body: request.body,
            });
        let (status, headers, body, body_must_not_be_polled) = match path.as_str() {
            "/oauth/token" | "/oauth2/token" | "/v1/oauth/token" => self.refresh_response(),
            "/backend-api/wham/usage" => self.codex_usage_response(),
            "/api/oauth/usage" => self.claude_usage_response(),
            "/v1/billing" => self.grok_billing_response(),
            "/v1/user" => self.grok_subscription_response(),
            "/backend-api/wham/rate-limit-reset-credits" => self.reset_credits_response(),
            "/backend-api/wham/rate-limit-reset-credits/consume" => self.consume_response().await,
            other => panic!("unexpected quota request path: {other}"),
        };
        let body: BoxByteStream = if body_must_not_be_polled {
            Box::pin(stream::poll_fn(|_| panic!("quota probe body was polled")))
        } else {
            Box::pin(stream::iter([Ok(body)]))
        };
        Ok(TransportResponse {
            status,
            headers,
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

type MockResponse = (StatusCode, HeaderMap, Bytes, bool);

impl QuotaTransport {
    fn refresh_response(&self) -> MockResponse {
        self.refresh_calls.fetch_add(1, Ordering::AcqRel);
        if self.authentication == AuthenticationMode::RefreshRejected {
            return service_unavailable();
        }
        ok(Bytes::from_static(
            br#"{"access_token":"new-access","expires_in":3600}"#,
        ))
    }

    fn codex_usage_response(&self) -> MockResponse {
        if let Some(rejected) = self.authentication_rejection() {
            return rejected;
        }
        ok(Bytes::from_static(
            br#"{"rate_limit":{"allowed":true,"limit_reached":false,"primary_window":{"used_percent":25.0,"limit_window_seconds":18000,"reset_after_seconds":60,"reset_at":1900000000},"secondary_window":null},"rate_limit_reset_credits":{"available_count":7}}"#,
        ))
    }

    fn claude_usage_response(&self) -> MockResponse {
        if let Some(rejected) = self.authentication_rejection() {
            return rejected;
        }
        ok(Bytes::from_static(
            br#"{"five_hour":{"utilization":12.5,"resets_at":"2030-01-01T01:00:00Z"},"seven_day":{"utilization":34.0,"resets_at":"2030-01-08T00:00:00Z"},"seven_day_sonnet":{"utilization":56.0,"resets_at":"2030-01-08T00:00:00Z"},"seven_day_overage_included":{"utilization":78.0,"resets_at":"2030-01-08T00:00:00Z"}}"#,
        ))
    }

    fn grok_billing_response(&self) -> MockResponse {
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

    fn grok_subscription_response(&self) -> MockResponse {
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

    fn reset_credits_response(&self) -> MockResponse {
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

    async fn consume_response(&self) -> MockResponse {
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
            AuthenticationMode::RefreshRejected => Some(unauthorized()),
        }
    }
}

fn header(request: &TransportRequest, name: &str) -> Option<String> {
    request
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn ok(body: Bytes) -> MockResponse {
    (StatusCode::OK, HeaderMap::new(), body, false)
}

fn unauthorized() -> MockResponse {
    (
        StatusCode::UNAUTHORIZED,
        HeaderMap::new(),
        Bytes::from_static(b"{}"),
        false,
    )
}

fn forbidden() -> MockResponse {
    (
        StatusCode::FORBIDDEN,
        HeaderMap::new(),
        Bytes::from_static(br#"{"code":"unauthorized:blocked-user"}"#),
        false,
    )
}

fn service_unavailable() -> MockResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        HeaderMap::new(),
        Bytes::from_static(b"{}"),
        false,
    )
}
