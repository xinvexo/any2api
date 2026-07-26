use std::sync::{Arc, Mutex};

#[cfg(test)]
use any2api_domain::ProxyProfile;
use any2api_domain::{ProxyKind, ProxyProfileId};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use rustls::pki_types::CertificateDer;
use tokio::time::timeout;

use super::{
    cache::ClientCache,
    construction::build_transport_client,
    failure::{failure_scope_for_unverified_path, map_send_error, validate_uri},
    pinned::PinnedClient,
    request_body::signaled_request_body,
};
use crate::{
    api::{
        BoxByteStream, TransportManager, TransportManagerConfig, TransportProxy, TransportRequest,
        TransportResponse,
    },
    error::{
        TransportConfigurationError, TransportError, TransportErrorStage, TransportFailureScope,
    },
    resolution::{ResolvedOrigin, resolve_origin},
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
    extra_root_certificates: Vec<CertificateDer<'static>>,
    clients: Mutex<ClientCache<TransportClientKey, TransportClient>>,
}

pub(crate) enum TransportClient {
    Reqwest(Client),
    Pinned(Box<PinnedClient>),
}

impl ReqwestTransportManager {
    pub fn new(config: TransportManagerConfig) -> Result<Self, TransportConfigurationError> {
        Self::new_inner(config, RUSTLS_NATIVE_ROOTS_POLICY_VERSION, Vec::new())
    }

    fn new_inner(
        config: TransportManagerConfig,
        tls_policy_version: u16,
        extra_root_certificates: Vec<CertificateDer<'static>>,
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
        certificate_der: impl AsRef<[u8]>,
    ) -> Result<Self, TransportConfigurationError> {
        Self::new_inner(
            config,
            TEST_EXTRA_ROOT_POLICY_VERSION,
            vec![CertificateDer::from(certificate_der.as_ref().to_vec())],
        )
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
    pub(crate) fn client_for(
        &self,
        profile: &ProxyProfile,
    ) -> Result<Arc<TransportClient>, TransportError> {
        self.client_for_resolved(TransportProxy::new(profile, None), None)
    }

    #[cfg(test)]
    pub(crate) fn client_for_proxy(
        &self,
        proxy: TransportProxy<'_>,
    ) -> Result<Arc<TransportClient>, TransportError> {
        self.client_for_resolved(proxy, None)
    }

    fn client_for_resolved(
        &self,
        proxy: TransportProxy<'_>,
        resolved_origin: Option<&ResolvedOrigin>,
    ) -> Result<Arc<TransportClient>, TransportError> {
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
        if let Some(client) = self
            .clients
            .lock()
            .expect("transport client cache lock poisoned")
            .get(&key)
        {
            return Ok(client);
        }
        // TLS root loading and client construction can take tens of
        // milliseconds; build outside the cache lock so concurrent cache hits
        // are never blocked, then let the first inserted client win.
        let client = Arc::new(build_transport_client(
            self.config,
            &self.extra_root_certificates,
            proxy,
            key.resolved_origin.clone().as_ref(),
        )?);
        Ok(self
            .clients
            .lock()
            .expect("transport client cache lock poisoned")
            .insert_if_absent(key, client))
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
        if let TransportClient::Pinned(client) = client.as_ref() {
            return client.execute(request).await;
        }
        let TransportClient::Reqwest(client) = client.as_ref() else {
            unreachable!("transport client variant was checked")
        };
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
                map_send_error(profile, uses_http_forward_proxy, error)
            }),
            signal = body_sent => {
                if signal.is_err() {
                    (&mut send).await.map_err(|error| {
                        map_send_error(profile, uses_http_forward_proxy, error)
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
                            map_send_error(profile, uses_http_forward_proxy, error)
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
