use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use axum::http::HeaderMap;
use ipnet::IpNet;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct AdminNetworkPolicy {
    trusted_proxies: Vec<IpNet>,
}

impl AdminNetworkPolicy {
    #[must_use]
    pub fn new(trusted_proxies: Vec<IpNet>) -> Self {
        Self { trusted_proxies }
    }

    #[must_use]
    pub fn direct_only() -> Self {
        Self::new(Vec::new())
    }

    pub fn resolve(
        &self,
        peer: Option<SocketAddr>,
        headers: &HeaderMap,
    ) -> Result<AdminConnection, AdminNetworkError> {
        let peer = peer.ok_or(AdminNetworkError::MissingPeer)?;
        if !self.is_trusted_proxy(peer.ip()) {
            return Ok(AdminConnection::direct(peer.ip()));
        }

        let client_ip =
            forwarded_client_ip(headers, peer.ip(), |address| self.is_trusted_proxy(address))?;
        let secure = forwarded_proto(headers)? == "https";
        Ok(AdminConnection {
            client_ip,
            secure,
            through_trusted_proxy: true,
        })
    }

    fn is_trusted_proxy(&self, address: IpAddr) -> bool {
        self.trusted_proxies
            .iter()
            .any(|network| network.contains(&address))
    }
}

impl Default for AdminNetworkPolicy {
    fn default() -> Self {
        Self::direct_only()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdminConnection {
    client_ip: IpAddr,
    secure: bool,
    through_trusted_proxy: bool,
}

impl AdminConnection {
    const fn direct(client_ip: IpAddr) -> Self {
        Self {
            client_ip,
            secure: false,
            through_trusted_proxy: false,
        }
    }

    pub const fn client_ip(self) -> IpAddr {
        self.client_ip
    }

    pub const fn is_loopback(self) -> bool {
        self.client_ip.is_loopback()
    }

    pub const fn is_secure(self) -> bool {
        self.secure
    }

    pub const fn through_trusted_proxy(self) -> bool {
        self.through_trusted_proxy
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum AdminNetworkError {
    #[error("request peer address is unavailable")]
    MissingPeer,
    #[error("trusted proxy headers are invalid")]
    InvalidForwardedHeaders,
}

fn forwarded_client_ip(
    headers: &HeaderMap,
    peer: IpAddr,
    is_trusted: impl Fn(IpAddr) -> bool,
) -> Result<IpAddr, AdminNetworkError> {
    let value = single_header(headers, "x-forwarded-for")?
        .to_str()
        .map_err(|_| AdminNetworkError::InvalidForwardedHeaders)?;
    let chain = value
        .split(',')
        .map(str::trim)
        .map(|value| {
            if value.is_empty() {
                return Err(AdminNetworkError::InvalidForwardedHeaders);
            }
            IpAddr::from_str(value).map_err(|_| AdminNetworkError::InvalidForwardedHeaders)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if chain.is_empty() {
        return Err(AdminNetworkError::InvalidForwardedHeaders);
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

fn forwarded_proto(headers: &HeaderMap) -> Result<&str, AdminNetworkError> {
    let value = single_header(headers, "x-forwarded-proto")?
        .to_str()
        .map_err(|_| AdminNetworkError::InvalidForwardedHeaders)?;
    if matches!(value, "http" | "https") {
        Ok(value)
    } else {
        Err(AdminNetworkError::InvalidForwardedHeaders)
    }
}

fn single_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a axum::http::HeaderValue, AdminNetworkError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .ok_or(AdminNetworkError::InvalidForwardedHeaders)?;
    if values.next().is_some() {
        return Err(AdminNetworkError::InvalidForwardedHeaders);
    }
    Ok(value)
}
