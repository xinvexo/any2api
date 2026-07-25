use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use axum::http::HeaderMap;
use ipnet::IpNet;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct ClientAddressPolicy {
    trusted_proxies: Vec<IpNet>,
}

impl ClientAddressPolicy {
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
    ) -> Result<ClientConnection, ClientAddressError> {
        let peer = peer.ok_or(ClientAddressError::MissingPeer)?;
        if !self.is_trusted_proxy(peer.ip()) {
            return Ok(ClientConnection::direct(peer.ip()));
        }

        let client_ip =
            forwarded_client_ip(headers, peer.ip(), |address| self.is_trusted_proxy(address))?;
        let secure = forwarded_proto(headers)? == "https";
        Ok(ClientConnection {
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

impl Default for ClientAddressPolicy {
    fn default() -> Self {
        Self::direct_only()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientConnection {
    client_ip: IpAddr,
    secure: bool,
    through_trusted_proxy: bool,
}

impl ClientConnection {
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
    let value = single_header(headers, "x-forwarded-for")?
        .to_str()
        .map_err(|_| ClientAddressError::InvalidForwardedHeaders)?;
    let chain = value
        .split(',')
        .map(str::trim)
        .map(|value| {
            if value.is_empty() {
                return Err(ClientAddressError::InvalidForwardedHeaders);
            }
            IpAddr::from_str(value).map_err(|_| ClientAddressError::InvalidForwardedHeaders)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if chain.is_empty() {
        return Err(ClientAddressError::InvalidForwardedHeaders);
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

fn forwarded_proto(headers: &HeaderMap) -> Result<&str, ClientAddressError> {
    let value = single_header(headers, "x-forwarded-proto")?
        .to_str()
        .map_err(|_| ClientAddressError::InvalidForwardedHeaders)?;
    if matches!(value, "http" | "https") {
        Ok(value)
    } else {
        Err(ClientAddressError::InvalidForwardedHeaders)
    }
}

fn single_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a axum::http::HeaderValue, ClientAddressError> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .ok_or(ClientAddressError::InvalidForwardedHeaders)?;
    if values.next().is_some() {
        return Err(ClientAddressError::InvalidForwardedHeaders);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, SocketAddr};

    use axum::http::{HeaderMap, HeaderValue};
    use ipnet::IpNet;

    use super::{ClientAddressError, ClientAddressPolicy};

    #[test]
    fn direct_peer_ignores_untrusted_forwarded_headers() {
        let headers = forwarded_headers("203.0.113.8", "https");
        let connection = ClientAddressPolicy::default()
            .resolve(Some(loopback_peer()), &headers)
            .expect("direct source");

        assert_eq!(connection.client_ip(), loopback_peer().ip());
        assert!(connection.is_loopback());
        assert!(!connection.is_secure());
        assert!(!connection.through_trusted_proxy());
    }

    #[test]
    fn trusted_chain_discards_a_spoofed_leftmost_address() {
        let policy = trusted_loopback_policy();
        let headers = forwarded_headers("127.0.0.1, 203.0.113.8", "https");
        let connection = policy
            .resolve(Some(loopback_peer()), &headers)
            .expect("trusted source");

        assert_eq!(connection.client_ip(), ip("203.0.113.8"));
        assert!(connection.is_secure());
        assert!(connection.through_trusted_proxy());
    }

    #[test]
    fn trusted_proxy_headers_fail_closed_when_missing_repeated_or_invalid() {
        let policy = trusted_loopback_policy();
        assert_eq!(
            policy.resolve(Some(loopback_peer()), &HeaderMap::new()),
            Err(ClientAddressError::InvalidForwardedHeaders)
        );

        let mut repeated = forwarded_headers("203.0.113.8", "https");
        repeated.append("x-forwarded-for", HeaderValue::from_static("198.51.100.2"));
        assert_eq!(
            policy.resolve(Some(loopback_peer()), &repeated),
            Err(ClientAddressError::InvalidForwardedHeaders)
        );

        let invalid = forwarded_headers("not-an-ip", "ftp");
        assert_eq!(
            policy.resolve(Some(loopback_peer()), &invalid),
            Err(ClientAddressError::InvalidForwardedHeaders)
        );
    }

    fn trusted_loopback_policy() -> ClientAddressPolicy {
        ClientAddressPolicy::new(vec!["127.0.0.0/8".parse::<IpNet>().expect("CIDR")])
    }

    fn forwarded_headers(addresses: &'static str, protocol: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static(addresses));
        headers.insert("x-forwarded-proto", HeaderValue::from_static(protocol));
        headers
    }

    fn loopback_peer() -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], 41_000))
    }

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("IP address")
    }
}
