use std::sync::{Arc, Mutex};

use any2api_domain::{ProxyKind, ProxyProfile, ProxyProfileId};
use async_trait::async_trait;
use futures_util::StreamExt;
use http::Uri;
use reqwest::{Certificate, Client, ClientBuilder, Proxy, redirect::Policy};

use crate::{
    api::{
        BoxByteStream, TransportManager, TransportManagerConfig, TransportRequest,
        TransportResponse,
    },
    client_cache::ClientCache,
    error::{TransportConfigurationError, TransportError, TransportErrorStage},
    origin_resolution::{ResolvedOrigin, resolve_origin},
    proxy_url::proxy_url,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TransportClientKey {
    proxy_id: ProxyProfileId,
    proxy_config_version: u64,
    proxy_kind: ProxyKind,
    policy: TransportClientPolicyKey,
    resolved_origin: Option<ResolvedOrigin>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TransportClientPolicyKey {
    connect_timeout: std::time::Duration,
    tls_policy_version: u16,
    http_version_policy_version: u16,
    pool_idle_timeout: std::time::Duration,
    pool_max_idle_per_host: usize,
    pool_policy_version: u16,
}

const RUSTLS_NATIVE_ROOTS_POLICY_VERSION: u16 = 1;
const HTTP_1_AND_2_POLICY_VERSION: u16 = 1;
const REQWEST_POOL_POLICY_VERSION: u16 = 1;
#[cfg(test)]
const TEST_EXTRA_ROOT_POLICY_VERSION: u16 = 2;

pub struct ReqwestTransportManager {
    config: TransportManagerConfig,
    policy: TransportClientPolicyKey,
    extra_root_certificates: Vec<Certificate>,
    clients: Mutex<ClientCache<TransportClientKey>>,
}

impl ReqwestTransportManager {
    pub fn new(config: TransportManagerConfig) -> Result<Self, TransportConfigurationError> {
        Self::new_inner(config, RUSTLS_NATIVE_ROOTS_POLICY_VERSION, Vec::new())
    }

    fn new_inner(
        config: TransportManagerConfig,
        tls_policy_version: u16,
        extra_root_certificates: Vec<Certificate>,
    ) -> Result<Self, TransportConfigurationError> {
        if config.max_cached_clients == 0 {
            return Err(TransportConfigurationError::EmptyClientCache);
        }
        Ok(Self {
            config,
            policy: TransportClientPolicyKey {
                connect_timeout: config.connect_timeout,
                tls_policy_version,
                http_version_policy_version: HTTP_1_AND_2_POLICY_VERSION,
                pool_idle_timeout: config.pool_idle_timeout,
                pool_max_idle_per_host: config.pool_max_idle_per_host,
                pool_policy_version: REQWEST_POOL_POLICY_VERSION,
            },
            extra_root_certificates,
            clients: Mutex::new(ClientCache::new(config.max_cached_clients)),
        })
    }

    #[cfg(test)]
    pub(crate) fn new_with_test_root_certificate(
        config: TransportManagerConfig,
        certificate: Certificate,
    ) -> Result<Self, TransportConfigurationError> {
        Self::new_inner(config, TEST_EXTRA_ROOT_POLICY_VERSION, vec![certificate])
    }

    #[must_use]
    pub fn config(&self) -> TransportManagerConfig {
        self.config
    }

    #[must_use]
    pub fn cached_client_count(&self) -> usize {
        self.clients
            .lock()
            .expect("transport client cache lock poisoned")
            .len()
    }

    #[cfg(test)]
    pub(crate) fn client_for(&self, profile: &ProxyProfile) -> Result<Arc<Client>, TransportError> {
        self.client_for_resolved(profile, None)
    }

    fn client_for_resolved(
        &self,
        profile: &ProxyProfile,
        resolved_origin: Option<&ResolvedOrigin>,
    ) -> Result<Arc<Client>, TransportError> {
        if !profile.enabled() {
            return Err(TransportError::proxy_unavailable(
                "configured proxy is disabled",
            ));
        }
        let key = TransportClientKey {
            proxy_id: profile.id(),
            proxy_config_version: profile.config_version(),
            proxy_kind: profile.kind(),
            policy: self.policy,
            resolved_origin: resolved_origin.cloned(),
        };
        let mut clients = self
            .clients
            .lock()
            .expect("transport client cache lock poisoned");
        let config = self.config;
        let extra_root_certificates = &self.extra_root_certificates;
        let resolved_origin = key.resolved_origin.clone();
        clients.get_or_insert_with(key, || {
            build_client(
                config,
                extra_root_certificates,
                profile,
                resolved_origin.as_ref(),
            )
        })
    }

    fn map_send_error(&self, profile: &ProxyProfile, error: reqwest::Error) -> TransportError {
        if error.is_connect() {
            if profile.kind() == ProxyKind::Direct {
                TransportError::new(
                    TransportErrorStage::Tcp,
                    any2api_domain::RetrySafety::DefinitelyNotSent,
                    "direct connection failed",
                )
            } else {
                TransportError::proxy_unavailable("configured proxy connection failed")
            }
        } else if error.is_timeout() {
            TransportError::new(
                TransportErrorStage::AwaitHeaders,
                any2api_domain::RetrySafety::Ambiguous,
                "upstream response headers timed out",
            )
        } else {
            TransportError::new(
                TransportErrorStage::AwaitHeaders,
                any2api_domain::RetrySafety::Ambiguous,
                "upstream request failed before response headers",
            )
        }
    }
}

impl Default for ReqwestTransportManager {
    fn default() -> Self {
        Self::new(TransportManagerConfig::default()).expect("default transport config is valid")
    }
}

#[async_trait]
impl TransportManager for ReqwestTransportManager {
    async fn execute(
        &self,
        proxy: &ProxyProfile,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        validate_uri(&request.uri)?;
        let resolved_origin =
            resolve_origin(&request.uri, request.network_policy, proxy.kind()).await?;
        let client = self.client_for_resolved(proxy, resolved_origin.as_ref())?;
        let response = client
            .request(request.method, request.uri.to_string())
            .headers(request.headers)
            .body(request.body)
            .send()
            .await
            .map_err(|error| self.map_send_error(proxy, error))?;
        let status = response.status();
        let headers = response.headers().clone();
        let body: BoxByteStream = Box::pin(response.bytes_stream().map(|result| {
            result.map_err(|_| {
                TransportError::new(
                    TransportErrorStage::ReadBody,
                    any2api_domain::RetrySafety::Ambiguous,
                    "upstream response body read failed",
                )
            })
        }));
        Ok(TransportResponse {
            status,
            headers,
            body,
        })
    }
}

fn build_client(
    config: TransportManagerConfig,
    extra_root_certificates: &[Certificate],
    profile: &ProxyProfile,
    resolved_origin: Option<&ResolvedOrigin>,
) -> Result<Client, TransportError> {
    let mut builder: ClientBuilder = Client::builder()
        .use_rustls_tls()
        .connect_timeout(config.connect_timeout)
        .pool_idle_timeout(config.pool_idle_timeout)
        .pool_max_idle_per_host(config.pool_max_idle_per_host)
        .redirect(Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy();
    if let Some(origin) = resolved_origin {
        builder = builder.resolve_to_addrs(&origin.host, origin.addresses.as_ref());
    }
    for certificate in extra_root_certificates {
        builder = builder.add_root_certificate(certificate.clone());
    }
    if let Some(url) = proxy_url(profile)? {
        let proxy = Proxy::all(url.as_str())
            .map_err(|_| TransportError::proxy_unavailable("configured proxy URL is invalid"))?;
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|_| {
        TransportError::new(
            if profile.kind() == ProxyKind::Direct {
                TransportErrorStage::Tcp
            } else {
                TransportErrorStage::ProxyHandshake
            },
            any2api_domain::RetrySafety::DefinitelyNotSent,
            "transport client construction failed",
        )
    })
}

fn validate_uri(uri: &Uri) -> Result<(), TransportError> {
    match uri.scheme_str() {
        Some("http" | "https") => Ok(()),
        _ => Err(TransportError::new(
            TransportErrorStage::WriteRequest,
            any2api_domain::RetrySafety::DefinitelyNotSent,
            "transport only supports HTTP and HTTPS upstream URIs",
        )),
    }
}
