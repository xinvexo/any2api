use std::str::FromStr;

use any2api_domain::{ProxyKind, ProxyProfile};
use bytes::Bytes;
use http::{HeaderMap, Method, Uri};

use crate::{
    api::{EndpointNetworkPolicy, TransportIsolationKey, TransportProxy, TransportRequest},
    client::ReqwestTransportManager,
};

use super::tests::{network_proxy, test_isolation};

#[test]
fn client_identity_is_decoupled_from_dns_but_pinned_per_origin() {
    let manager = ReqwestTransportManager::default();
    let direct = ProxyProfile::direct();
    let isolation = test_isolation();

    manager
        .warm_client_for_request(
            TransportProxy::new(&direct, None),
            &strict_request_to("https://one.invalid/v1", isolation),
        )
        .expect("first strict direct client");
    manager
        .warm_client_for_request(
            TransportProxy::new(&direct, None),
            &strict_request_to("https://two.invalid/v1", isolation),
        )
        .expect("second strict direct client");
    assert_eq!(manager.cached_client_count(), 1);

    manager
        .warm_client_for_request(
            TransportProxy::new(&direct, None),
            &request_to("https://three.invalid/v1", isolation),
        )
        .expect("plain direct client");
    assert_eq!(manager.cached_client_count(), 2);

    let proxy = network_proxy(
        "Pinned",
        ProxyKind::Http,
        "127.0.0.1:8080".parse().expect("proxy address"),
        true,
    );
    manager
        .warm_client_for_request(
            TransportProxy::new(&proxy, None),
            &strict_request_to("https://one.invalid/v1", isolation),
        )
        .expect("first pinned client");
    manager
        .warm_client_for_request(
            TransportProxy::new(&proxy, None),
            &strict_request_to("https://two.invalid/v1", isolation),
        )
        .expect("second pinned client");
    assert_eq!(manager.cached_client_count(), 4);
}

fn strict_request_to(uri: &str, isolation: TransportIsolationKey) -> TransportRequest {
    TransportRequest {
        network_policy: EndpointNetworkPolicy::new().with_strict_ssrf(true),
        ..request_to(uri, isolation)
    }
}

fn request_to(uri: &str, isolation: TransportIsolationKey) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        isolation,
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: std::time::Duration::from_secs(15),
    }
}
