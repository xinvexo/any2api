use any2api_domain::ProxyKind;
use reqwest::{Certificate, Client, ClientBuilder, Proxy, redirect::Policy};
use rustls::pki_types::CertificateDer;

use super::{pinned::PinnedClient, reqwest::TransportClient};
use crate::{
    api::{TransportManagerConfig, TransportProxy},
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    proxy::url::proxy_url,
    resolution::ResolvedOrigin,
};

pub(super) fn build_transport_client(
    config: TransportManagerConfig,
    extra_root_certificates: &[CertificateDer<'static>],
    proxy: TransportProxy<'_>,
    resolved_origin: Option<&ResolvedOrigin>,
) -> Result<TransportClient, TransportError> {
    validate_proxy_credentials(proxy)?;
    if proxy.profile().kind() != ProxyKind::Direct
        && let Some(origin) = resolved_origin
    {
        return PinnedClient::build(config, extra_root_certificates, proxy, origin)
            .map(Box::new)
            .map(TransportClient::Pinned);
    }
    build_reqwest_client(config, extra_root_certificates, proxy, resolved_origin)
        .map(TransportClient::Reqwest)
}

fn build_reqwest_client(
    config: TransportManagerConfig,
    extra_root_certificates: &[CertificateDer<'static>],
    proxy: TransportProxy<'_>,
    resolved_origin: Option<&ResolvedOrigin>,
) -> Result<Client, TransportError> {
    let profile = proxy.profile();
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
        builder =
            builder.add_root_certificate(Certificate::from_der(certificate.as_ref()).map_err(
                |_| TransportError::configuration("configured TLS root certificate is invalid"),
            )?);
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
