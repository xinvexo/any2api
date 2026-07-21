use std::{str::FromStr, time::Duration};

use any2api_domain::{ProxyKind, RetrySafety};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use tokio::net::TcpListener;

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, ProxyCredentials, TransportManager, TransportManagerConfig,
        TransportProxy, TransportRequest,
    },
    error::{TransportErrorStage, TransportFailureScope},
    http_connect_tests as connect, reqwest_manager_tests as plain,
};

#[tokio::test]
async fn strict_http_proxy_uses_a_pinned_ip_and_preserves_host() {
    let (proxy_address, request) =
        plain::spawn_http_response(StatusCode::OK, HeaderMap::new(), "strict-proxy").await;
    let manager = ReqwestTransportManager::default();
    let proxy = plain::network_proxy("HTTP", ProxyKind::Http, proxy_address, true);
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to("http://localhost:43123/v1/test?mode=strict", true),
        )
        .await
        .expect("strict HTTP proxy response");

    assert_eq!(
        plain::collect_body(response).await,
        Bytes::from_static(b"strict-proxy")
    );
    let request = request.await.expect("captured strict proxy request");
    assert!(request.starts_with("GET http://"));
    assert!(!request.starts_with("GET http://localhost"));
    assert!(request.contains("/v1/test?mode=strict HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("host: localhost:43123")
    );
}

#[tokio::test]
async fn strict_socks5_uses_an_ip_target_and_preserves_host() {
    let (proxy_address, target, request) = plain::spawn_socks5_response("strict-socks").await;
    let manager = ReqwestTransportManager::default();
    let proxy = plain::network_proxy("SOCKS5", ProxyKind::Socks5, proxy_address, true);
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to("http://localhost:80/socks", true),
        )
        .await
        .expect("strict SOCKS response");

    assert_eq!(
        plain::collect_body(response).await,
        Bytes::from_static(b"strict-socks")
    );
    let target = target.await.expect("SOCKS target");
    assert!(!target.contains("localhost"));
    assert!(target.ends_with(":80"));
    assert!(
        request
            .await
            .expect("captured SOCKS request")
            .to_ascii_lowercase()
            .contains("host: localhost")
    );
}

#[tokio::test]
async fn strict_proxy_rejects_private_dns_before_connecting_to_the_proxy() {
    let proxy_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy listener");
    let proxy_address = proxy_listener.local_addr().expect("proxy address");
    let manager = ReqwestTransportManager::default();
    let proxy = plain::network_proxy("HTTP", ProxyKind::Http, proxy_address, true);

    let error = match manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to("http://localhost:43123/blocked", false),
        )
        .await
    {
        Ok(_) => panic!("strict private DNS must be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::Dns);
    assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), proxy_listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn strict_https_connect_pins_ip_preserves_sni_host_and_proxy_auth() {
    let identity = connect::TestTlsIdentity::generate();
    let (origin_address, origin_request) =
        connect::spawn_https_response(identity.server_config, StatusCode::OK, "strict-tunnel")
            .await;
    let (proxy_address, connect_request) = connect::spawn_connect_proxy(origin_address).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = connect::network_proxy(proxy_address)
        .set_authentication("proxy-user")
        .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "proxy-password".to_owned());

    let response = manager
        .execute(
            TransportProxy::new(&proxy, Some(&credentials)),
            strict_request_to(
                &format!("https://localhost:{}/strict-tunnel", origin_address.port()),
                true,
            ),
        )
        .await
        .expect("strict HTTPS response through HTTP proxy");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        connect::collect_body(response).await,
        Bytes::from_static(b"strict-tunnel")
    );
    let connect = connect_request.await.expect("captured CONNECT request");
    assert!(connect.starts_with("CONNECT "));
    assert!(!connect.starts_with("CONNECT localhost"));
    assert!(connect.contains("Proxy-Authorization: Basic"));
    assert!(
        origin_request
            .await
            .expect("captured origin request")
            .to_ascii_lowercase()
            .contains(&format!("host: localhost:{}", origin_address.port()))
    );
}

#[tokio::test]
async fn strict_connect_attributes_endpoint_tls_failure_to_the_endpoint() {
    let identity = connect::TestTlsIdentity::generate();
    let origin_address = connect::spawn_tls_handshake_endpoint(identity.server_config).await;
    let (proxy_address, _connect_request) = connect::spawn_connect_proxy(origin_address).await;
    let manager = ReqwestTransportManager::default();
    let proxy = connect::network_proxy(proxy_address);

    let error = match manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to(
                &format!(
                    "https://localhost:{}/untrusted-certificate",
                    origin_address.port()
                ),
                true,
            ),
        )
        .await
    {
        Ok(_) => panic!("untrusted endpoint certificate must fail"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::Tls);
    assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
}

#[tokio::test]
async fn strict_connect_407_is_a_rejected_proxy_handshake() {
    let origin = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("origin listener");
    let origin_address = origin.local_addr().expect("origin address");
    let (proxy_address, _connect_request) =
        connect::spawn_rejecting_connect_proxy(StatusCode::PROXY_AUTHENTICATION_REQUIRED).await;
    let manager = ReqwestTransportManager::default();
    let proxy = connect::network_proxy(proxy_address);

    let error = match manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to(
                &format!("https://localhost:{}/rejected", origin_address.port()),
                true,
            ),
        )
        .await
    {
        Ok(_) => panic!("CONNECT 407 must fail in transport"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::Proxy);
    assert_eq!(error.retry_safety, RetrySafety::RejectedBeforeExecution);
}

fn strict_request_to(uri: &str, allow_private_network: bool) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        network_policy: EndpointNetworkPolicy::new(allow_private_network).with_strict_ssrf(true),
        read_timeout: Duration::from_secs(15),
    }
}
