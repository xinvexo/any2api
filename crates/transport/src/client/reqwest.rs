use std::sync::{Arc, Mutex, OnceLock};

#[cfg(test)]
use any2api_domain::ProxyProfile;
use any2api_domain::{ProxyKind, ProxyProfileId};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use rustls::pki_types::CertificateDer;
use tokio::time::Instant;

use super::{
    body_timeout::timeout_body,
    cache::ClientCache,
    construction::build_transport_client,
    deadline::await_response_headers,
    failure::{
        connect_timeout_error, failure_scope_for_unverified_path, map_send_error, validate_uri,
    },
    pinned::PinnedClient,
    request_body::signaled_request_body,
};
use crate::{
    api::{
        BoxByteStream, EndpointNetworkPolicy, TransportIsolationKey, TransportManager,
        TransportManagerConfig, TransportProxy, TransportRequest, TransportRequestDiagnostics,
        TransportResponse,
    },
    connection::TlsConfigFactory,
    error::{
        TransportConfigurationError, TransportError, TransportErrorStage, TransportFailureScope,
    },
    profile::{GENERIC_GATEWAY_TRANSPORT_PROFILE, TransportWireProfile},
    resolution::{OriginTarget, origin_target},
    response_coding::{decode_response_content, sanitize_request_accept_encoding},
};

/// Client identity never includes resolved addresses: DNS rotation must reuse
/// the cached client and its warm connection pool instead of rebuilding both.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TransportClientKey {
    isolation: TransportIsolationKey,
    proxy_id: ProxyProfileId,
    proxy_config_version: u64,
    proxy_kind: ProxyKind,
    policy: TransportClientPolicyKey,
    strict_direct_dns: bool,
    pinned_origin: Option<OriginTarget>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct TransportClientPolicyKey {
    wire_profile_version: u16,
    connect_timeout: std::time::Duration,
    tls_policy_version: u16,
    http_version_policy_version: u16,
    pool_idle_timeout: std::time::Duration,
    pool_max_idle_per_host: usize,
    pool_policy_version: u16,
}

#[cfg(test)]
const TEST_EXTRA_ROOT_POLICY_VERSION: u16 = 2;

pub struct ReqwestTransportManager {
    config: TransportManagerConfig,
    policy: TransportClientPolicyKey,
    extra_root_certificates: Vec<CertificateDer<'static>>,
    tls_factory: OnceLock<TlsConfigFactory>,
    clients: Mutex<ClientCache<TransportClientKey, TransportClient>>,
}

pub(crate) enum TransportClient {
    Reqwest(Client),
    Pinned(Box<PinnedClient>),
}

impl ReqwestTransportManager {
    pub fn new(config: TransportManagerConfig) -> Result<Self, TransportConfigurationError> {
        Self::new_inner(
            config,
            GENERIC_GATEWAY_TRANSPORT_PROFILE.tls_policy_version(),
            Vec::new(),
        )
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
                wire_profile_version: GENERIC_GATEWAY_TRANSPORT_PROFILE.policy_version(),
                connect_timeout: config.connect_timeout,
                tls_policy_version,
                http_version_policy_version: GENERIC_GATEWAY_TRANSPORT_PROFILE
                    .http_version_policy_version(),
                pool_idle_timeout: config.pool_idle_timeout,
                pool_max_idle_per_host: config.pool_max_idle_per_host,
                pool_policy_version: GENERIC_GATEWAY_TRANSPORT_PROFILE.pool_policy_version(),
            },
            extra_root_certificates,
            tls_factory: OnceLock::new(),
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
    pub const fn wire_profile(&self) -> &'static TransportWireProfile {
        &GENERIC_GATEWAY_TRANSPORT_PROFILE
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
        isolation: TransportIsolationKey,
    ) -> Result<Arc<TransportClient>, TransportError> {
        self.client_for_policy(TransportProxy::new(profile, None), isolation, None, false)
    }

    #[cfg(test)]
    pub(crate) fn client_for_proxy(
        &self,
        proxy: TransportProxy<'_>,
        isolation: TransportIsolationKey,
    ) -> Result<Arc<TransportClient>, TransportError> {
        self.client_for_policy(proxy, isolation, None, false)
    }

    #[cfg(test)]
    pub(crate) fn warm_client_for_request(
        &self,
        proxy: TransportProxy<'_>,
        request: &TransportRequest,
    ) -> Result<(), TransportError> {
        let (pinned_origin, strict_direct_dns) =
            client_selector(proxy.profile().kind(), request.network_policy, &request.uri)?;
        self.client_for_policy(
            proxy,
            request.isolation,
            pinned_origin.as_ref(),
            strict_direct_dns,
        )?;
        Ok(())
    }

    fn client_for_policy(
        &self,
        proxy: TransportProxy<'_>,
        isolation: TransportIsolationKey,
        pinned_origin: Option<&OriginTarget>,
        strict_direct_dns: bool,
    ) -> Result<Arc<TransportClient>, TransportError> {
        let profile = proxy.profile();
        if !profile.enabled() {
            return Err(TransportError::configuration(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                "configured proxy is disabled",
            ));
        }
        // Warm the shared trust roots before taking the cache lock so
        // the one-time trust store load never blocks concurrent cache users.
        let tls_factory = self.tls_factory()?;
        if isolation.is_ephemeral() {
            let tls_config = tls_factory.build();
            return Ok(Arc::new(build_transport_client(
                self.config,
                &tls_config,
                proxy,
                pinned_origin,
                strict_direct_dns,
            )?));
        }
        let key = TransportClientKey {
            isolation,
            proxy_id: profile.id(),
            proxy_config_version: profile.config_version(),
            proxy_kind: profile.kind(),
            policy: self.policy,
            strict_direct_dns,
            pinned_origin: pinned_origin.cloned(),
        };
        let mut clients = self
            .clients
            .lock()
            .expect("transport client cache lock poisoned");
        clients.retain(|cached| !isolation.retires(cached.isolation));
        if let Some(client) = clients.get(&key) {
            return Ok(client);
        }
        // Construction is cheap once the TLS roots are cached, so building
        // under the lock doubles as singleflight: concurrent callers of the
        // same key never build duplicate clients or connection pools.
        let tls_config = tls_factory.build();
        let client = Arc::new(build_transport_client(
            self.config,
            &tls_config,
            proxy,
            pinned_origin,
            strict_direct_dns,
        )?);
        Ok(clients.insert_if_absent(key, client))
    }

    fn tls_factory(&self) -> Result<&TlsConfigFactory, TransportError> {
        if let Some(factory) = self.tls_factory.get() {
            return Ok(factory);
        }
        let factory = TlsConfigFactory::new(&self.extra_root_certificates)?;
        Ok(self.tls_factory.get_or_init(|| factory))
    }
}

/// Picks the client flavor for a request: proxied strict-SSRF requests use a
/// per-origin pinned client, strict direct requests pin through the caching
/// DNS resolver, and everything else shares a plain client per proxy profile.
fn client_selector(
    proxy_kind: ProxyKind,
    policy: EndpointNetworkPolicy,
    uri: &http::Uri,
) -> Result<(Option<OriginTarget>, bool), TransportError> {
    if !policy.strict_ssrf() {
        return Ok((None, false));
    }
    if proxy_kind == ProxyKind::Direct {
        return Ok((None, true));
    }
    Ok((Some(origin_target(uri)?), false))
}

impl Default for ReqwestTransportManager {
    fn default() -> Self {
        Self::new(TransportManagerConfig::default()).expect("default transport config is valid")
    }
}

#[async_trait]
impl TransportManager for ReqwestTransportManager {
    fn request_diagnostics(
        &self,
        proxy: TransportProxy<'_>,
        request: &TransportRequest,
    ) -> Option<TransportRequestDiagnostics> {
        Some(crate::diagnostics::for_request(self.config, proxy, request))
    }

    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        mut request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        let connect_deadline = Instant::now() + self.config.connect_timeout;
        let profile = proxy.profile();
        validate_uri(&request.uri)?;
        let uses_http_forward_proxy =
            profile.kind() == ProxyKind::Http && request.uri.scheme_str() == Some("http");
        let connect_timeout_error = connect_timeout_error(profile, uses_http_forward_proxy);
        let body_failure_scope = failure_scope_for_unverified_path(profile);
        let read_timeout = request.read_timeout;
        let (pinned_origin, strict_direct_dns) =
            client_selector(profile.kind(), request.network_policy, &request.uri)?;
        let client = self.client_for_policy(
            proxy,
            request.isolation,
            pinned_origin.as_ref(),
            strict_direct_dns,
        )?;
        if Instant::now() >= connect_deadline {
            return Err(connect_timeout_error);
        }
        if let TransportClient::Pinned(client) = client.as_ref() {
            return client
                .execute(request, connect_deadline, connect_timeout_error)
                .await;
        }
        let TransportClient::Reqwest(client) = client.as_ref() else {
            unreachable!("transport client variant was checked")
        };
        sanitize_request_accept_encoding(&mut request.headers);
        let (body, body_sent) = signaled_request_body(request.body);
        let send = client
            .request(request.method, request.uri.to_string())
            .headers(request.headers)
            .body(body)
            .send();
        let response = await_response_headers(
            send,
            body_sent,
            connect_deadline,
            read_timeout,
            |error| map_send_error(profile, uses_http_forward_proxy, error),
            connect_timeout_error,
            TransportError::new(
                TransportErrorStage::AwaitHeaders,
                body_failure_scope,
                any2api_domain::RetrySafety::Ambiguous,
                "upstream response headers timed out",
            ),
        )
        .await?;
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
        decode_response_content(TransportResponse {
            status,
            headers,
            body: timeout_body(body, read_timeout, body_failure_scope),
            read_failure_scope: body_failure_scope,
        })
    }
}
