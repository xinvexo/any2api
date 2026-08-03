use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};

use any2api_domain::is_loopback_ip;
use any2api_runtime::api::PublishedSnapshot;
use axum::http::HeaderMap;
use ipnet::IpNet;
use thiserror::Error;

pub(crate) use any2api_domain::canonical_ip;

fn resolve(
    trusted_proxies: &[IpNet],
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Result<ClientConnection, ClientAddressError> {
    let peer_ip = canonical_ip(peer.ok_or(ClientAddressError::MissingPeer)?.ip());
    if !is_trusted_proxy(trusted_proxies, peer_ip) {
        return Ok(ClientConnection::direct(peer_ip));
    }

    let client_ip = forwarded_client_ip(headers, peer_ip, |address| {
        is_trusted_proxy(trusted_proxies, address)
    })?;
    let secure = forwarded_is_secure(headers)?;
    Ok(ClientConnection {
        client_ip,
        secure,
        through_trusted_proxy: true,
    })
}

/// Immutable ingress result shared by logging and every authentication path.
#[derive(Clone)]
pub(crate) struct ClientAddressContext {
    snapshot: Arc<PublishedSnapshot>,
    peer_ip: Option<IpAddr>,
    connection: Result<ClientConnection, ClientAddressError>,
}

impl ClientAddressContext {
    pub(crate) fn capture(
        snapshot: Arc<PublishedSnapshot>,
        peer: Option<SocketAddr>,
        headers: &HeaderMap,
    ) -> Self {
        let peer_ip = peer.map(|address| canonical_ip(address.ip()));
        let connection = resolve(
            snapshot.settings().network().trusted_proxy_cidrs(),
            peer,
            headers,
        );
        Self {
            snapshot,
            peer_ip,
            connection,
        }
    }

    pub(crate) fn snapshot(&self) -> &PublishedSnapshot {
        &self.snapshot
    }

    pub(crate) fn snapshot_arc(&self) -> Arc<PublishedSnapshot> {
        Arc::clone(&self.snapshot)
    }

    pub(crate) const fn connection(&self) -> Result<ClientConnection, ClientAddressError> {
        self.connection
    }

    pub(crate) fn audit_client_ip(&self) -> Option<IpAddr> {
        self.connection
            .map(ClientConnection::client_ip)
            .ok()
            .or(self.peer_ip)
    }
}

fn is_trusted_proxy(trusted_proxies: &[IpNet], address: IpAddr) -> bool {
    trusted_proxies
        .iter()
        .any(|network| network.contains(&address))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientConnection {
    client_ip: IpAddr,
    secure: bool,
    through_trusted_proxy: bool,
}

impl ClientConnection {
    fn direct(client_ip: IpAddr) -> Self {
        Self {
            client_ip,
            secure: false,
            through_trusted_proxy: false,
        }
    }

    pub const fn client_ip(self) -> IpAddr {
        self.client_ip
    }

    pub const fn is_secure(self) -> bool {
        self.secure
    }

    pub const fn through_trusted_proxy(self) -> bool {
        self.through_trusted_proxy
    }

    pub fn is_direct_loopback(self) -> bool {
        !self.through_trusted_proxy && is_loopback_ip(self.client_ip)
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ClientAddressError {
    #[error("request peer address is unavailable")]
    MissingPeer,
    #[error("trusted proxy headers are invalid")]
    InvalidForwardedHeaders,
}

fn forwarded_client_ip(
    headers: &HeaderMap,
    peer: IpAddr,
    is_trusted: impl Fn(IpAddr) -> bool,
) -> Result<IpAddr, ClientAddressError> {
    let mut values = headers.get_all("x-forwarded-for").iter().peekable();
    if values.peek().is_none() {
        return Ok(peer);
    }
    let mut chain = Vec::new();
    for value in values {
        let value = value
            .to_str()
            .map_err(|_| ClientAddressError::InvalidForwardedHeaders)?;
        for address in value.split(',').map(str::trim) {
            if address.is_empty() {
                return Err(ClientAddressError::InvalidForwardedHeaders);
            }
            chain.push(
                IpAddr::from_str(address)
                    .map(canonical_ip)
                    .map_err(|_| ClientAddressError::InvalidForwardedHeaders)?,
            );
        }
    }
    let mut client = peer;
    for address in chain.into_iter().rev() {
        if !is_trusted(client) {
            break;
        }
        client = address;
    }
    Ok(client)
}

fn forwarded_is_secure(headers: &HeaderMap) -> Result<bool, ClientAddressError> {
    let mut values = headers.get_all("x-forwarded-proto").iter();
    let Some(value) = values.next() else {
        return Ok(false);
    };
    if values.next().is_some() {
        return Err(ClientAddressError::InvalidForwardedHeaders);
    }
    match value
        .to_str()
        .map_err(|_| ClientAddressError::InvalidForwardedHeaders)?
    {
        "http" => Ok(false),
        "https" => Ok(true),
        _ => Err(ClientAddressError::InvalidForwardedHeaders),
    }
}

#[cfg(test)]
mod tests;
