use std::{fmt, pin::Pin, result::Result as StdResult, time::Duration};

use any2api_domain::ProxyProfile;
use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use http::{HeaderMap, Method, StatusCode, Uri};

pub use crate::client::ReqwestTransportManager;
pub use crate::error::{
    TransportConfigurationError, TransportError, TransportErrorStage, TransportFailureScope,
};
pub use crate::{
    diagnostics::{TransportRequestDiagnostics, TransportResolverMode},
    isolation::{TransportIsolationKey, TransportTrafficClass},
    profile::{GENERIC_GATEWAY_TRANSPORT_PROFILE, TransportWireProfile},
    proxy::ProxyCredentials,
};

pub type BoxByteStream =
    Pin<Box<dyn Stream<Item = StdResult<Bytes, TransportError>> + Send + 'static>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EndpointNetworkPolicy {
    strict_ssrf: bool,
}

impl EndpointNetworkPolicy {
    #[must_use]
    pub const fn new() -> Self {
        Self { strict_ssrf: false }
    }

    #[must_use]
    pub const fn with_strict_ssrf(mut self, strict_ssrf: bool) -> Self {
        self.strict_ssrf = strict_ssrf;
        self
    }

    /// Strict SSRF is address *pinning*: the transport resolves the upstream
    /// host locally and connects to exactly those addresses (also through
    /// proxies), defeating DNS-rebinding between check and use. It never
    /// filters loopback or private ranges because upstream endpoints may
    /// legitimately live there (for example local LLM servers).
    #[must_use]
    pub const fn strict_ssrf(self) -> bool {
        self.strict_ssrf
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportManagerConfig {
    pub connect_timeout: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub max_cached_clients: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportRuntimeSnapshot {
    cache_entries: usize,
    cache_capacity: usize,
    cache_hits: u64,
    cache_misses: u64,
    cache_evictions: u64,
}

impl TransportRuntimeSnapshot {
    #[must_use]
    pub const fn new(
        cache_entries: usize,
        cache_capacity: usize,
        cache_hits: u64,
        cache_misses: u64,
        cache_evictions: u64,
    ) -> Self {
        Self {
            cache_entries,
            cache_capacity,
            cache_hits,
            cache_misses,
            cache_evictions,
        }
    }

    #[must_use]
    pub const fn cache_entries(self) -> usize {
        self.cache_entries
    }

    #[must_use]
    pub const fn cache_capacity(self) -> usize {
        self.cache_capacity
    }

    #[must_use]
    pub const fn cache_hits(self) -> u64 {
        self.cache_hits
    }

    #[must_use]
    pub const fn cache_misses(self) -> u64 {
        self.cache_misses
    }

    #[must_use]
    pub const fn cache_evictions(self) -> u64 {
        self.cache_evictions
    }
}

impl Default for TransportManagerConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            // Below the common 60s NAT silent-drop window so pooled
            // connections are reaped before a middlebox kills them.
            pool_idle_timeout: Duration::from_secs(50),
            pool_max_idle_per_host: 8,
            max_cached_clients: 64,
        }
    }
}

#[derive(Clone)]
pub struct TransportRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub isolation: TransportIsolationKey,
    pub network_policy: EndpointNetworkPolicy,
    pub read_timeout: Duration,
}

impl fmt::Debug for TransportRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportRequest")
            .field("method", &self.method)
            .field("uri", &self.uri)
            .field("header_count", &self.headers.len())
            .field("body_bytes", &self.body.len())
            .field("isolation", &self.isolation)
            .field("read_timeout", &self.read_timeout)
            .finish()
    }
}

pub struct TransportResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: BoxByteStream,
    pub read_failure_scope: TransportFailureScope,
}

#[derive(Clone, Copy)]
pub struct TransportProxy<'a> {
    profile: &'a ProxyProfile,
    credentials: Option<&'a ProxyCredentials>,
}

impl<'a> TransportProxy<'a> {
    #[must_use]
    pub const fn new(profile: &'a ProxyProfile, credentials: Option<&'a ProxyCredentials>) -> Self {
        Self {
            profile,
            credentials,
        }
    }

    #[must_use]
    pub const fn profile(self) -> &'a ProxyProfile {
        self.profile
    }

    #[must_use]
    pub const fn credentials(self) -> Option<&'a ProxyCredentials> {
        self.credentials
    }
}

impl fmt::Debug for TransportProxy<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportProxy")
            .field("proxy_profile_id", &self.profile.id())
            .field("authentication_configured", &self.credentials.is_some())
            .finish()
    }
}

#[async_trait]
pub trait TransportManager: Send + Sync {
    /// Returns aggregate pool state when the adapter has a read-only snapshot.
    fn runtime_snapshot(&self) -> Option<TransportRuntimeSnapshot> {
        None
    }

    /// Returns a secret-free snapshot of the policy that would be used for
    /// this request. Custom adapters may opt out; the production manager does
    /// not. The snapshot is observational and must not mutate client caches.
    fn request_diagnostics(
        &self,
        _proxy: TransportProxy<'_>,
        _request: &TransportRequest,
    ) -> Option<TransportRequestDiagnostics> {
        None
    }

    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError>;
}
