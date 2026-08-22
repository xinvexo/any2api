//! Test transport for OAuth quota and refresh behavior.

mod responses;

use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
};

use any2api_transport::api::{
    BoxByteStream, TransportFailureScope, TransportManager, TransportProxy, TransportRequest,
    TransportResponse, TransportTrafficClass,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream;
use http::{Method, header::AUTHORIZATION};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum AuthenticationMode {
    Accepted,
    RejectOnce,
    AlwaysReject,
    AlwaysForbidden,
    CodexCloudflareBlocked,
    CodexUnknownForbidden,
    RefreshRejected,
    RefreshInvalidGrant,
    RefreshTokenReused,
}

pub(super) struct CapturedQuotaRequest {
    pub(super) method: Method,
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
    pub(super) traffic_class: TransportTrafficClass,
    pub(super) body: Bytes,
}

pub(super) struct QuotaTransport {
    available_count: AtomicU32,
    authentication: AuthenticationMode,
    usage_calls: AtomicUsize,
    model_catalog_calls: AtomicUsize,
    refresh_calls: AtomicUsize,
    consume_calls: AtomicUsize,
    block_consume: bool,
    grok_unified: bool,
    codex_exhausted: AtomicBool,
    codex_has_credits: AtomicBool,
    large_model_catalog: AtomicBool,
    codex_reset_at: i64,
    block_next_usage: AtomicBool,
    usage_started: Semaphore,
    usage_release: Semaphore,
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
            model_catalog_calls: AtomicUsize::new(0),
            refresh_calls: AtomicUsize::new(0),
            consume_calls: AtomicUsize::new(0),
            block_consume,
            grok_unified: false,
            codex_exhausted: AtomicBool::new(false),
            codex_has_credits: AtomicBool::new(false),
            large_model_catalog: AtomicBool::new(false),
            codex_reset_at: unix_now().saturating_add(60),
            block_next_usage: AtomicBool::new(false),
            usage_started: Semaphore::new(0),
            usage_release: Semaphore::new(0),
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

    pub(super) fn new_blocking_usage(available_count: u32) -> Self {
        let transport = Self::new(available_count, AuthenticationMode::Accepted, false);
        transport.block_next_usage.store(true, Ordering::Release);
        transport
    }

    pub(super) fn captured(&self) -> Vec<CapturedQuotaRequest> {
        self.captured
            .lock()
            .expect("captured request lock")
            .iter()
            .map(|request| CapturedQuotaRequest {
                method: request.method.clone(),
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
                traffic_class: request.traffic_class,
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

    pub(super) fn model_catalog_calls(&self) -> usize {
        self.model_catalog_calls.load(Ordering::Acquire)
    }

    pub(super) async fn wait_for_consume(&self) {
        self.consume_started
            .acquire()
            .await
            .expect("consume start signal")
            .forget();
    }

    pub(super) async fn wait_for_usage(&self) {
        self.usage_started
            .acquire()
            .await
            .expect("usage start signal")
            .forget();
    }

    pub(super) fn release_usage(&self) {
        self.usage_release.add_permits(1);
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

    pub(super) fn set_codex_exhausted(&self, exhausted: bool) {
        self.codex_exhausted.store(exhausted, Ordering::Release);
    }

    pub(super) fn set_codex_has_credits(&self, has_credits: bool) {
        self.codex_has_credits.store(has_credits, Ordering::Release);
    }

    pub(super) fn set_large_model_catalog(&self) {
        self.large_model_catalog.store(true, Ordering::Release);
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
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
                method: request.method.clone(),
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
                traffic_class: request.isolation.traffic_class(),
                body: request.body,
            });
        if path == "/backend-api/wham/usage" && self.block_next_usage.swap(false, Ordering::AcqRel)
        {
            self.usage_started.add_permits(1);
            self.usage_release
                .acquire()
                .await
                .expect("usage release signal")
                .forget();
        }
        let (status, headers, body) = match path.as_str() {
            "/oauth/token" | "/oauth2/token" | "/v1/oauth/token" => self.refresh_response(),
            "/backend-api/wham/usage" => self.codex_usage_response(),
            "/backend-api/codex/models" => self.codex_model_catalog_response(),
            "/api/oauth/usage" => self.claude_usage_response(),
            "/v1/models" => self.openai_model_catalog_response(),
            "/v1/billing" => self.grok_billing_response(),
            "/v1/user" => self.grok_subscription_response(),
            "/backend-api/wham/rate-limit-reset-credits" => self.reset_credits_response(),
            "/backend-api/wham/rate-limit-reset-credits/consume" => self.consume_response().await,
            other => panic!("unexpected quota request path: {other}"),
        };
        let body: BoxByteStream = Box::pin(stream::iter([Ok(body)]));
        Ok(TransportResponse {
            status,
            headers,
            body,
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

impl QuotaTransport {
    fn codex_model_catalog_response(&self) -> (http::StatusCode, http::HeaderMap, Bytes) {
        self.model_catalog_calls.fetch_add(1, Ordering::AcqRel);
        if self.large_model_catalog.load(Ordering::Acquire) {
            let padding = "x".repeat(128 * 1024);
            return (
                http::StatusCode::OK,
                http::HeaderMap::new(),
                Bytes::from(format!(
                    r#"{{"models":[{{"slug":"gpt-catalog-a","supported_in_api":true}}],"padding":"{padding}"}}"#
                )),
            );
        }
        (
            http::StatusCode::OK,
            http::HeaderMap::new(),
            Bytes::from_static(
                br#"{"models":[{"slug":"gpt-catalog-a","supported_in_api":true},{"slug":"chatgpt-only","supported_in_api":false}]}"#,
            ),
        )
    }

    fn openai_model_catalog_response(&self) -> (http::StatusCode, http::HeaderMap, Bytes) {
        self.model_catalog_calls.fetch_add(1, Ordering::AcqRel);
        (
            http::StatusCode::OK,
            http::HeaderMap::new(),
            Bytes::from_static(br#"{"data":[{"id":"catalog-model"}]}"#),
        )
    }
}

fn header(request: &TransportRequest, name: &str) -> Option<String> {
    request
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}
