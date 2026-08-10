use std::sync::Arc;

use any2api_domain::ProxyKind;
use reqwest::{Client, ClientBuilder, Proxy, redirect::Policy};
use rustls::ClientConfig;

use super::{dns::CachedDnsResolver, pinned::PinnedClient, reqwest::TransportClient};
use crate::{
    api::{TransportManagerConfig, TransportProxy},
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    profile::GENERIC_GATEWAY_TRANSPORT_PROFILE as WIRE_PROFILE,
    proxy::url::proxy_url,
    resolution::OriginTarget,
};

pub(super) fn build_transport_client(
    config: TransportManagerConfig,
    tls_config: &ClientConfig,
    proxy: TransportProxy<'_>,
    pinned_origin: Option<&OriginTarget>,
    strict_direct_dns: bool,
) -> Result<TransportClient, TransportError> {
    validate_proxy_credentials(proxy)?;
    if let Some(origin) = pinned_origin {
        return PinnedClient::build(config, tls_config.clone(), proxy, origin)
            .map(Box::new)
            .map(TransportClient::Pinned);
    }
    build_reqwest_client(config, tls_config, proxy, strict_direct_dns).map(TransportClient::Reqwest)
}

fn build_reqwest_client(
    config: TransportManagerConfig,
    tls_config: &ClientConfig,
    proxy: TransportProxy<'_>,
    strict_direct_dns: bool,
) -> Result<Client, TransportError> {
    let profile = proxy.profile();
    let mut tls_config = tls_config.clone();
    tls_config.alpn_protocols = WIRE_PROFILE.owned_alpn_protocols();
    let mut builder: ClientBuilder = Client::builder()
        .use_preconfigured_tls(tls_config)
        .connect_timeout(config.connect_timeout)
        .pool_idle_timeout(config.pool_idle_timeout)
        .pool_max_idle_per_host(config.pool_max_idle_per_host)
        .tcp_keepalive(WIRE_PROFILE.tcp_keep_alive_interval())
        .http2_keep_alive_interval(WIRE_PROFILE.http2_keep_alive_interval())
        .http2_keep_alive_timeout(WIRE_PROFILE.http2_keep_alive_timeout())
        .http2_keep_alive_while_idle(WIRE_PROFILE.http2_keep_alive_while_idle())
        .no_proxy();
    if !WIRE_PROFILE.redirects_enabled() {
        builder = builder.redirect(Policy::none());
    }
    if !WIRE_PROFILE.automatic_request_retries() {
        builder = builder.retry(reqwest::retry::never());
    }
    if strict_direct_dns {
        builder = builder.dns_resolver(Arc::new(CachedDnsResolver));
    }
    if let Some(url) = proxy_url(profile)? {
        let mut reqwest_proxy = Proxy::all(url.as_str()).map_err(|_| {
            TransportError::configuration(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                "configured proxy URL is invalid",
            )
        })?;
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
            TransportErrorStage::ProxyHandshake,
            TransportFailureScope::Proxy,
            "configured proxy authentication material is inconsistent",
        )),
    }
}
