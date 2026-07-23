use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use any2api_domain::{ProxyKind, RetrySafety};
use http::Uri;
use tokio::net::lookup_host;

use crate::{
    api::EndpointNetworkPolicy,
    error::{TransportError, TransportErrorStage, TransportFailureScope},
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ResolvedOrigin {
    pub(crate) host: Arc<str>,
    pub(crate) port: u16,
    pub(crate) secure: bool,
    pub(crate) addresses: Arc<[SocketAddr]>,
}

pub(crate) async fn resolve_origin(
    uri: &Uri,
    policy: EndpointNetworkPolicy,
    proxy_kind: ProxyKind,
) -> Result<Option<ResolvedOrigin>, TransportError> {
    if proxy_kind != ProxyKind::Direct && !policy.strict_ssrf() {
        return Ok(None);
    }
    let host = uri.host().ok_or_else(|| {
        TransportError::new(
            TransportErrorStage::Dns,
            TransportFailureScope::Endpoint,
            RetrySafety::DefinitelyNotSent,
            "upstream URI has no host",
        )
    })?;
    let port = uri
        .port_u16()
        .or_else(|| match uri.scheme_str() {
            Some("http") => Some(80),
            Some("https") => Some(443),
            _ => None,
        })
        .ok_or_else(|| {
            TransportError::new(
                TransportErrorStage::Dns,
                TransportFailureScope::Endpoint,
                RetrySafety::DefinitelyNotSent,
                "upstream URI has no port",
            )
        })?;

    if let Ok(address) = host.parse::<IpAddr>() {
        if proxy_kind == ProxyKind::Direct {
            return Ok(None);
        }
        return Ok(Some(ResolvedOrigin {
            host: Arc::from(host.to_owned()),
            port,
            secure: uri.scheme_str() == Some("https"),
            addresses: Arc::from(vec![SocketAddr::new(address, port)].into_boxed_slice()),
        }));
    }

    let mut addresses = lookup_host((host, port))
        .await
        .map_err(|_| {
            TransportError::new(
                TransportErrorStage::Dns,
                TransportFailureScope::Endpoint,
                RetrySafety::DefinitelyNotSent,
                "upstream DNS resolution failed",
            )
        })?
        .collect::<Vec<_>>();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        return Err(TransportError::new(
            TransportErrorStage::Dns,
            TransportFailureScope::Endpoint,
            RetrySafety::DefinitelyNotSent,
            "upstream DNS resolution returned no addresses",
        ));
    }
    Ok(Some(ResolvedOrigin {
        host: Arc::from(host.to_owned()),
        port,
        secure: uri.scheme_str() == Some("https"),
        addresses: Arc::from(addresses.into_boxed_slice()),
    }))
}
