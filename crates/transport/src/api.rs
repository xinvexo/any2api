use std::{fmt, pin::Pin, result::Result as StdResult, time::Duration};

use any2api_domain::ProxyProfile;
use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use http::{HeaderMap, Method, StatusCode, Uri};

pub use crate::{
    ReqwestTransportManager, TransportConfigurationError, TransportError, TransportErrorStage,
    TransportFailureScope, proxy_credentials::ProxyCredentials,
};

pub type BoxByteStream =
    Pin<Box<dyn Stream<Item = StdResult<Bytes, TransportError>> + Send + 'static>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EndpointNetworkPolicy {
    allow_private_network: bool,
    strict_ssrf: bool,
}

impl EndpointNetworkPolicy {
    #[must_use]
    pub const fn new(allow_private_network: bool) -> Self {
        Self {
            allow_private_network,
            strict_ssrf: false,
        }
    }

    #[must_use]
    pub const fn with_strict_ssrf(mut self, strict_ssrf: bool) -> Self {
        self.strict_ssrf = strict_ssrf;
        self
    }

    #[must_use]
    pub const fn allow_private_network(self) -> bool {
        self.allow_private_network
    }

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

impl Default for TransportManagerConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            pool_idle_timeout: Duration::from_secs(90),
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
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError>;
}
