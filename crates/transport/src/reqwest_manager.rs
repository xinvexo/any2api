use std::{
    error::Error as _,
    sync::{Arc, Mutex},
};

use any2api_domain::{ProxyKind, ProxyProfile, ProxyProfileId};
use async_trait::async_trait;
use futures_util::StreamExt;
use http::Uri;
use reqwest::{Certificate, Client, ClientBuilder, Proxy, redirect::Policy};
use tokio::time::timeout;

use crate::{
    api::{
        BoxByteStream, TransportManager, TransportManagerConfig, TransportProxy, TransportRequest,
        TransportResponse,
    },
    client_cache::ClientCache,
    error::{
        TransportConfigurationError, TransportError, TransportErrorStage, TransportFailureScope,
    },
    origin_resolution::{ResolvedOrigin, resolve_origin},
    proxy_url::proxy_url,
    request_body::signaled_request_body,
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
        self.client_for_resolved(TransportProxy::new(profile, None), None)
    }

    #[cfg(test)]
    pub(crate) fn client_for_proxy(
        &self,
        proxy: TransportProxy<'_>,
    ) -> Result<Arc<Client>, TransportError> {
        self.client_for_resolved(proxy, None)
    }

    fn client_for_resolved(
        &self,
        proxy: TransportProxy<'_>,
        resolved_origin: Option<&ResolvedOrigin>,
    ) -> Result<Arc<Client>, TransportError> {
        let profile = proxy.profile();
        if !profile.enabled() {
            return Err(TransportError::configuration(
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
                proxy,
                resolved_origin.as_ref(),
            )
        })
    }

    fn map_send_error(
        &self,
        profile: &ProxyProfile,
        uses_http_forward_proxy: bool,
        error: reqwest::Error,
    ) -> TransportError {
        if error.is_connect() {
            if profile.kind() == ProxyKind::Http && is_proxy_authentication_error(&error) {
                TransportError::proxy_unavailable("configured proxy authentication was rejected")
            } else if profile.kind() == ProxyKind::Direct {
                TransportError::new(
                    TransportErrorStage::Tcp,
                    TransportFailureScope::Endpoint,
                    any2api_domain::RetrySafety::DefinitelyNotSent,
                    "direct connection failed",
                )
            } else if uses_http_forward_proxy {
                TransportError::proxy_unavailable("configured proxy connection failed")
            } else {
                TransportError::new(
                    TransportErrorStage::ProxyHandshake,
                    TransportFailureScope::Unattributed,
                    any2api_domain::RetrySafety::DefinitelyNotSent,
                    "proxied connection failed",
                )
            }
        } else if error.is_timeout() {
            TransportError::new(
                TransportErrorStage::AwaitHeaders,
                failure_scope_for_unverified_path(profile),
                any2api_domain::RetrySafety::Ambiguous,
                "upstream response headers timed out",
            )
        } else {
            TransportError::new(
                TransportErrorStage::AwaitHeaders,
                failure_scope_for_unverified_path(profile),
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
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        let profile = proxy.profile();
        validate_uri(&request.uri)?;
        let uses_http_forward_proxy =
            profile.kind() == ProxyKind::Http && request.uri.scheme_str() == Some("http");
        let body_failure_scope = failure_scope_for_unverified_path(profile);
        let read_timeout = request.read_timeout;
        let resolved_origin =
            resolve_origin(&request.uri, request.network_policy, profile.kind()).await?;
        let client = self.client_for_resolved(proxy, resolved_origin.as_ref())?;
        let (body, body_sent) = signaled_request_body(request.body);
        let send = client
            .request(request.method, request.uri.to_string())
            .headers(request.headers)
            .body(body)
            .send();
        tokio::pin!(send);
        let response = tokio::select! {
            biased;
            result = &mut send => result.map_err(|error| {
                self.map_send_error(profile, uses_http_forward_proxy, error)
            }),
            signal = body_sent => {
                if signal.is_err() {
                    (&mut send).await.map_err(|error| {
                        self.map_send_error(profile, uses_http_forward_proxy, error)
                    })
                } else {
                    timeout(read_timeout, &mut send)
                        .await
                        .map_err(|_| {
                            TransportError::new(
                                TransportErrorStage::AwaitHeaders,
                                body_failure_scope,
                                any2api_domain::RetrySafety::Ambiguous,
                                "upstream response headers timed out",
                            )
                        })?
                        .map_err(|error| {
                            self.map_send_error(profile, uses_http_forward_proxy, error)
                        })
                }
            }
        }?;
        let status = response.status();
        if uses_http_forward_proxy && status == reqwest::StatusCode::PROXY_AUTHENTICATION_REQUIRED {
            return Err(TransportError::new(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                any2api_domain::RetrySafety::RejectedBeforeExecution,
                "configured proxy authentication was rejected",
            ));
        }
        let headers = response.headers().clone();
        let body: BoxByteStream = Box::pin(response.bytes_stream().map(move |result| {
            result.map_err(|_| {
                TransportError::new(
                    TransportErrorStage::ReadBody,
                    body_failure_scope,
                    any2api_domain::RetrySafety::Ambiguous,
                    "upstream response body read failed",
                )
            })
        }));
        Ok(TransportResponse {
            status,
            headers,
            body,
            read_failure_scope: body_failure_scope,
        })
    }
}

// reqwest 0.12.28 does not re-export hyper-util's TunnelError, so the typed
// CONNECT 407 case is identified through its stable source display and pinned by a real test.
fn is_proxy_authentication_error(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(cause) = source {
        if cause.to_string() == "tunnel error: proxy authorization required" {
            return true;
        }
        source = cause.source();
    }
    false
}

fn build_client(
    config: TransportManagerConfig,
    extra_root_certificates: &[Certificate],
    proxy: TransportProxy<'_>,
    resolved_origin: Option<&ResolvedOrigin>,
) -> Result<Client, TransportError> {
    let profile = proxy.profile();
    validate_proxy_credentials(proxy)?;
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
        let mut reqwest_proxy = Proxy::all(url.as_str())
            .map_err(|_| TransportError::configuration("configured proxy URL is invalid"))?;
        if let Some(credentials) = proxy.credentials() {
            reqwest_proxy =
                reqwest_proxy.basic_auth(credentials.username(), credentials.password());
        }
        builder = builder.proxy(reqwest_proxy);
    }
    builder.build().map_err(|_| {
        TransportError::new(
            if profile.kind() == ProxyKind::Direct {
                TransportErrorStage::Tcp
            } else {
                TransportErrorStage::ProxyHandshake
            },
            TransportFailureScope::Unattributed,
            any2api_domain::RetrySafety::DefinitelyNotSent,
            "transport client construction failed",
        )
    })
}

fn validate_proxy_credentials(proxy: TransportProxy<'_>) -> Result<(), TransportError> {
    let profile = proxy.profile();
    match (profile.authentication(), proxy.credentials()) {
        (None, None) => Ok(()),
        (Some(metadata), Some(credentials)) if metadata.username() == credentials.username() => {
            Ok(())
        }
        _ => Err(TransportError::configuration(
            "configured proxy authentication material is inconsistent",
        )),
    }
}

fn validate_uri(uri: &Uri) -> Result<(), TransportError> {
    match uri.scheme_str() {
        Some("http" | "https") => Ok(()),
        _ => Err(TransportError::new(
            TransportErrorStage::WriteRequest,
            TransportFailureScope::Unattributed,
            any2api_domain::RetrySafety::DefinitelyNotSent,
            "transport only supports HTTP and HTTPS upstream URIs",
        )),
    }
}

fn failure_scope_for_unverified_path(profile: &ProxyProfile) -> TransportFailureScope {
    if profile.kind() == ProxyKind::Direct {
        TransportFailureScope::Endpoint
    } else {
        TransportFailureScope::Unattributed
    }
}
