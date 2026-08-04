use std::net::{IpAddr, SocketAddr};

use axum::http::{HeaderMap, HeaderValue};
use ipnet::IpNet;

use super::{ClientAddressError, resolve};

#[test]
fn direct_peer_ignores_untrusted_forwarded_headers() {
    let headers = forwarded_headers("203.0.113.8", "https");
    let connection = resolve(&[], Some(loopback_peer()), &headers).expect("direct source");

    assert_eq!(connection.client_ip(), loopback_peer().ip());
    assert!(connection.client_ip().is_loopback());
    assert!(!connection.is_secure());
    assert!(!connection.through_trusted_proxy());
    assert!(connection.is_direct_loopback());
}

#[test]
fn trusted_chain_discards_a_spoofed_leftmost_address() {
    let trusted = trusted_loopback_networks();
    let headers = forwarded_headers("127.0.0.1, 203.0.113.8", "https");
    let connection = resolve(&trusted, Some(loopback_peer()), &headers).expect("trusted source");

    assert_eq!(connection.client_ip(), ip("203.0.113.8"));
    assert!(connection.is_secure());
    assert!(connection.through_trusted_proxy());
    assert!(!connection.is_direct_loopback());
}

#[test]
fn missing_forwarded_headers_use_the_proxy_peer_and_insecure_transport() {
    let connection = resolve(
        &trusted_loopback_networks(),
        Some(loopback_peer()),
        &HeaderMap::new(),
    )
    .expect("missing headers have conservative defaults");

    assert_eq!(connection.client_ip(), loopback_peer().ip());
    assert!(!connection.is_secure());
    assert!(connection.through_trusted_proxy());
    assert!(!connection.is_direct_loopback());
}

#[test]
fn each_forwarded_header_can_fall_back_independently() {
    let trusted = trusted_loopback_networks();
    let mut only_for = HeaderMap::new();
    only_for.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.8"));
    let connection = resolve(&trusted, Some(loopback_peer()), &only_for).expect("missing proto");
    assert_eq!(connection.client_ip(), ip("203.0.113.8"));
    assert!(!connection.is_secure());

    let mut only_proto = HeaderMap::new();
    only_proto.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    let connection =
        resolve(&trusted, Some(loopback_peer()), &only_proto).expect("missing client chain");
    assert_eq!(connection.client_ip(), loopback_peer().ip());
    assert!(connection.is_secure());
    assert!(connection.through_trusted_proxy());
}

#[test]
fn repeated_forwarded_for_lines_form_one_ordered_proxy_chain() {
    let trusted = vec!["10.0.0.0/8".parse::<IpNet>().expect("CIDR")];
    let peer = SocketAddr::from(([10, 0, 0, 1], 41_000));
    let mut headers = HeaderMap::new();
    headers.append("x-forwarded-for", HeaderValue::from_static("198.51.100.2"));
    headers.append("x-forwarded-for", HeaderValue::from_static("10.0.0.2"));
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));

    let connection = resolve(&trusted, Some(peer), &headers).expect("multi-line XFF chain");

    assert_eq!(connection.client_ip(), ip("198.51.100.2"));
    assert!(connection.is_secure());
}

#[test]
fn present_but_invalid_forwarded_headers_fail_closed() {
    let trusted = trusted_loopback_networks();
    for addresses in ["", "not-an-ip", "203.0.113.8, "] {
        let headers = forwarded_headers(addresses, "https");
        assert_eq!(
            resolve(&trusted, Some(loopback_peer()), &headers),
            Err(ClientAddressError::InvalidForwardedHeaders)
        );
    }

    for protocol in ["", "ftp", ",https", "wss, https"] {
        let headers = forwarded_headers("203.0.113.8", protocol);
        assert_eq!(
            resolve(&trusted, Some(loopback_peer()), &headers),
            Err(ClientAddressError::InvalidForwardedHeaders)
        );
    }
}

#[test]
fn multi_hop_forwarded_proto_accepts_consistent_values_and_rejects_conflicts() {
    let trusted = trusted_loopback_networks();
    for (protocol, secure) in [
        ("https,https", true),
        ("http, http", false),
        (" HTTPS ", true),
        ("HTTP", false),
    ] {
        let headers = forwarded_headers("203.0.113.8", protocol);
        let connection =
            resolve(&trusted, Some(loopback_peer()), &headers).expect("multi-hop proto chain");
        assert_eq!(connection.is_secure(), secure, "protocol: {protocol:?}");
    }

    let mut repeated_proto = forwarded_headers("203.0.113.8", "https");
    repeated_proto.append("x-forwarded-proto", HeaderValue::from_static("HTTPS"));
    let connection = resolve(&trusted, Some(loopback_peer()), &repeated_proto)
        .expect("consistent repeated proto headers are accepted");
    assert!(connection.is_secure());

    for conflicting in ["https,http", "http, https"] {
        let headers = forwarded_headers("203.0.113.8", conflicting);
        assert_eq!(
            resolve(&trusted, Some(loopback_peer()), &headers),
            Err(ClientAddressError::InvalidForwardedHeaders),
            "protocol: {conflicting:?}"
        );
    }
    let mut conflicting_repeat = forwarded_headers("203.0.113.8", "https");
    conflicting_repeat.append("x-forwarded-proto", HeaderValue::from_static("http"));
    assert_eq!(
        resolve(&trusted, Some(loopback_peer()), &conflicting_repeat),
        Err(ClientAddressError::InvalidForwardedHeaders)
    );
}

#[test]
fn any_invalid_repeated_forwarded_for_line_rejects_the_whole_chain() {
    let trusted = trusted_loopback_networks();
    let mut headers = forwarded_headers("203.0.113.8", "https");
    headers.append("x-forwarded-for", HeaderValue::from_static("invalid"));

    assert_eq!(
        resolve(&trusted, Some(loopback_peer()), &headers),
        Err(ClientAddressError::InvalidForwardedHeaders)
    );
}

#[test]
fn ipv4_mapped_loopback_is_canonicalized_before_loopback_checks() {
    let peer = "[::ffff:127.0.0.1]:41000"
        .parse::<SocketAddr>()
        .expect("mapped loopback peer");
    let connection = resolve(&[], Some(peer), &HeaderMap::new()).expect("direct mapped loopback");

    assert_eq!(connection.client_ip(), ip("127.0.0.1"));
    assert!(connection.client_ip().is_loopback());
    assert!(!connection.through_trusted_proxy());
    assert!(connection.is_direct_loopback());
}

#[test]
fn ipv4_mapped_proxy_and_forwarded_client_use_canonical_ipv4() {
    let trusted = vec!["10.0.0.0/8".parse::<IpNet>().expect("CIDR")];
    let peer = "[::ffff:10.0.0.1]:41000"
        .parse::<SocketAddr>()
        .expect("mapped proxy peer");
    let headers = forwarded_headers("::ffff:198.51.100.9", "https");
    let connection = resolve(&trusted, Some(peer), &headers).expect("trusted mapped proxy");

    assert_eq!(connection.client_ip(), ip("198.51.100.9"));
    assert!(connection.is_secure());
    assert!(connection.through_trusted_proxy());
    assert!(!connection.is_direct_loopback());
}

#[test]
fn trusted_loopback_proxy_never_grants_direct_loopback_access() {
    let trusted = trusted_loopback_networks();
    let headers = forwarded_headers("127.0.0.1", "http");
    let connection =
        resolve(&trusted, Some(loopback_peer()), &headers).expect("trusted loopback proxy");

    assert!(connection.client_ip().is_loopback());
    assert!(connection.through_trusted_proxy());
    assert!(!connection.is_direct_loopback());
}

fn trusted_loopback_networks() -> Vec<IpNet> {
    vec!["127.0.0.0/8".parse::<IpNet>().expect("CIDR")]
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
