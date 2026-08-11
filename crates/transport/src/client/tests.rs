use std::{str::FromStr, sync::Arc, time::Duration};

use any2api_domain::{
    CredentialId, ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId, RetrySafety,
    RoutingCredentialId,
};
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, Method, StatusCode, Uri, header::AUTHORIZATION};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use crate::{
    ReqwestTransportManager,
    api::EndpointNetworkPolicy,
    api::{
        TransportIsolationKey, TransportManager, TransportManagerConfig, TransportProxy,
        TransportRequest, TransportResolverMode, TransportTrafficClass,
    },
    error::{TransportErrorStage, TransportFailureScope},
};

#[tokio::test]
async fn direct_transport_reaches_the_origin() {
    let (address, request) = spawn_http_response(StatusCode::OK, HeaderMap::new(), "direct").await;
    let manager = ReqwestTransportManager::default();
    let proxy = ProxyProfile::direct();
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to(&format!("http://localhost:{}/direct", address.port())),
        )
        .await
        .expect("direct response");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(collect_body(response).await, Bytes::from_static(b"direct"));
    assert!(
        request
            .await
            .expect("captured request")
            .starts_with("GET /direct HTTP/1.1")
    );
}

#[tokio::test]
async fn direct_transport_accepts_private_origin_without_extra_authorization() {
    let (address, request) = spawn_http_response(StatusCode::OK, HeaderMap::new(), "private").await;
    let manager = ReqwestTransportManager::default();
    let proxy = ProxyProfile::direct();
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to(&format!("http://{address}/private")),
        )
        .await
        .expect("private origin response");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(collect_body(response).await, Bytes::from_static(b"private"));
    assert!(
        request
            .await
            .expect("captured private request")
            .starts_with("GET /private HTTP/1.1")
    );
}

#[tokio::test]
async fn http_proxy_receives_the_absolute_upstream_uri() {
    let (proxy_address, request) =
        spawn_http_response(StatusCode::OK, HeaderMap::new(), "proxied").await;
    let manager = ReqwestTransportManager::default();
    let proxy = network_proxy("HTTP", ProxyKind::Http, proxy_address, true);
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to("http://upstream.invalid/v1/test?mode=proxy"),
        )
        .await
        .expect("proxy response");

    assert_eq!(collect_body(response).await, Bytes::from_static(b"proxied"));
    assert!(
        request
            .await
            .expect("captured proxy request")
            .starts_with("GET http://upstream.invalid/v1/test?mode=proxy HTTP/1.1")
    );
}

#[tokio::test]
async fn socks5_uses_remote_dns_and_carries_the_http_request() {
    let (proxy_address, target, request) = spawn_socks5_response("socks").await;
    let manager = ReqwestTransportManager::default();
    let proxy = network_proxy("SOCKS5", ProxyKind::Socks5, proxy_address, true);
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to("http://remote-dns.invalid/socks"),
        )
        .await
        .expect("SOCKS response");

    assert_eq!(collect_body(response).await, Bytes::from_static(b"socks"));
    assert_eq!(target.await.expect("SOCKS target"), "remote-dns.invalid:80");
    assert!(
        request
            .await
            .expect("captured SOCKS request")
            .starts_with("GET /socks HTTP/1.1")
    );
}

#[tokio::test]
async fn unavailable_explicit_proxy_fails_closed_without_reaching_origin() {
    let origin = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("origin listener");
    let origin_address = origin.local_addr().expect("origin address");
    let (unavailable_address, connect_request) = spawn_rejecting_connect_proxy().await;
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        connect_timeout: Duration::from_millis(500),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let proxy = network_proxy("Unavailable", ProxyKind::Http, unavailable_address, true);
    let request = request_to(&format!("https://{origin_address}/must-not-connect"));
    manager
        .client_for(&proxy, request.isolation)
        .expect("warm proxy client");

    let error = match manager
        .execute(TransportProxy::new(&proxy, None), request)
        .await
    {
        Ok(_) => panic!("explicit proxy failure must not use DIRECT"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::EgressPath);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
    assert!(
        connect_request
            .await
            .expect("captured CONNECT request")
            .starts_with(&format!("CONNECT {origin_address} HTTP/1.1"))
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), origin.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn redirects_are_returned_without_following_the_location() {
    let redirected = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("redirect target");
    let redirected_address = redirected.local_addr().expect("redirect address");
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::LOCATION,
        format!("http://{redirected_address}/followed")
            .parse()
            .expect("location header"),
    );
    let (address, _request) = spawn_http_response(StatusCode::FOUND, headers, "redirect").await;
    let manager = ReqwestTransportManager::default();

    let proxy = ProxyProfile::direct();
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to(&format!("http://{address}/redirect")),
        )
        .await
        .expect("redirect response");

    assert_eq!(response.status, StatusCode::FOUND);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), redirected.accept())
            .await
            .is_err()
    );
}

#[test]
fn client_cache_is_bounded_and_separates_updated_proxy_versions() {
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        max_cached_clients: 1,
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let original = network_proxy(
        "Proxy",
        ProxyKind::Http,
        "127.0.0.1:8080".parse().expect("proxy address"),
        true,
    );
    let isolation = test_isolation();
    let first = manager
        .client_for(&original, isolation)
        .expect("first client");
    let reused = manager
        .client_for(&original, isolation)
        .expect("reused client");
    assert!(Arc::ptr_eq(&first, &reused));

    let updated = original
        .updated(
            ProxyDraft::new(
                "Proxy",
                ProxyKind::Http,
                ProxyAddress::new("127.0.0.1", 8081).expect("updated address"),
                true,
            )
            .expect("updated draft"),
        )
        .expect("updated proxy");
    let next = manager
        .client_for(&updated, isolation)
        .expect("next generation client");
    assert!(!Arc::ptr_eq(&first, &next));
    assert_eq!(manager.cached_client_count(), 1);

    let rebuilt = manager
        .client_for(&original, isolation)
        .expect("rebuilt old client");
    assert!(!Arc::ptr_eq(&first, &rebuilt));
    assert_eq!(manager.cached_client_count(), 1);
}

#[test]
fn newer_credential_generation_retires_older_cached_clients() {
    let manager = ReqwestTransportManager::default();
    let proxy = ProxyProfile::direct();
    let credential_id = RoutingCredentialId::provider_credential(CredentialId::new());
    let old = TransportIsolationKey::routing_credential(
        credential_id,
        1,
        1,
        TransportTrafficClass::DataPlane,
    );
    let next = TransportIsolationKey::routing_credential(
        credential_id,
        1,
        2,
        TransportTrafficClass::DataPlane,
    );
    let old_client = manager.client_for(&proxy, old).expect("old client");
    assert_eq!(manager.cached_client_count(), 1);

    let next_client = manager.client_for(&proxy, next).expect("next client");

    assert!(!Arc::ptr_eq(&old_client, &next_client));
    assert_eq!(Arc::strong_count(&old_client), 1);
    assert_eq!(manager.cached_client_count(), 1);
    let quota = TransportIsolationKey::routing_credential(
        credential_id,
        1,
        2,
        TransportTrafficClass::OAuthQuota,
    );
    manager.client_for(&proxy, quota).expect("quota client");
    assert_eq!(manager.cached_client_count(), 2);
}

#[test]
fn request_debug_never_contains_authorization_values() {
    let mut request = request_to("https://api.example.com/v1/responses");
    request.headers.insert(
        AUTHORIZATION,
        "Bearer must-not-leak".parse().expect("authorization"),
    );

    assert!(!format!("{request:?}").contains("must-not-leak"));
}

#[test]
fn request_diagnostics_report_final_resolver_proxy_and_timeout_policy() {
    let config = TransportManagerConfig {
        connect_timeout: Duration::from_secs(7),
        pool_idle_timeout: Duration::from_secs(41),
        ..TransportManagerConfig::default()
    };
    let manager = ReqwestTransportManager::new(config).expect("transport manager");
    let direct = ProxyProfile::direct();
    let mut request = request_to("https://upstream.invalid/v1/responses");
    request.read_timeout = Duration::from_secs(123);

    let system = manager
        .request_diagnostics(TransportProxy::new(&direct, None), &request)
        .expect("production diagnostics");
    assert_eq!(system.wire_profile_id(), "generic-rustls-hyper-v3");
    assert_eq!(system.wire_profile_version(), 3);
    assert_eq!(system.timeout_policy_version(), 1);
    assert_eq!(system.resolver_mode(), TransportResolverMode::System);
    assert_eq!(system.proxy_kind(), ProxyKind::Direct);
    assert_eq!(system.connect_timeout(), Duration::from_secs(7));
    assert_eq!(system.read_timeout(), Duration::from_secs(123));
    assert_eq!(system.pool_idle_timeout(), Duration::from_secs(41));

    request.network_policy = EndpointNetworkPolicy::new().with_strict_ssrf(true);
    let local = manager
        .request_diagnostics(TransportProxy::new(&direct, None), &request)
        .expect("strict diagnostics");
    assert_eq!(local.resolver_mode(), TransportResolverMode::LocalCached);

    request.network_policy = EndpointNetworkPolicy::new();
    let http = network_proxy(
        "HTTP",
        ProxyKind::Http,
        "127.0.0.1:8080".parse().expect("proxy address"),
        true,
    );
    let remote = manager
        .request_diagnostics(TransportProxy::new(&http, None), &request)
        .expect("proxy diagnostics");
    assert_eq!(remote.resolver_mode(), TransportResolverMode::ProxyRemote);
    assert_eq!(remote.proxy_kind(), ProxyKind::Http);
}

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        isolation: test_isolation(),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

pub(crate) fn test_isolation() -> TransportIsolationKey {
    TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic)
}

pub(crate) fn network_proxy(
    name: &str,
    kind: ProxyKind,
    address: std::net::SocketAddr,
    enabled: bool,
) -> ProxyProfile {
    let address =
        ProxyAddress::new(address.ip().to_string(), address.port()).expect("proxy address");
    let draft = ProxyDraft::new(name, kind, address, enabled).expect("proxy draft");
    ProxyProfile::create(ProxyProfileId::new(), draft).expect("proxy profile")
}

pub(crate) async fn collect_body(mut response: crate::api::TransportResponse) -> Bytes {
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next().await {
        body.extend_from_slice(&chunk.expect("response body chunk"));
    }
    body.into()
}

pub(crate) async fn spawn_http_response(
    status: StatusCode,
    headers: HeaderMap,
    body: &'static str,
) -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP listener");
    let address = listener.local_addr().expect("HTTP address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("HTTP connection");
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        write_http_response(&mut stream, status, &headers, body).await;
    });
    (address, request_rx)
}

pub(crate) async fn spawn_socks5_response(
    body: &'static str,
) -> (
    std::net::SocketAddr,
    oneshot::Receiver<String>,
    oneshot::Receiver<String>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("SOCKS listener");
    let address = listener.local_addr().expect("SOCKS address");
    let (target_tx, target_rx) = oneshot::channel();
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("SOCKS connection");
        let target = accept_socks5(&mut stream).await;
        target_tx.send(target).ok();
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        write_http_response(&mut stream, StatusCode::OK, &HeaderMap::new(), body).await;
    });
    (address, target_rx, request_rx)
}

async fn spawn_rejecting_connect_proxy() -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP proxy listener");
    let address = listener.local_addr().expect("HTTP proxy address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("proxy connection");
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        stream
            .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
            .await
            .expect("CONNECT rejection");
    });
    (address, request_rx)
}

async fn accept_socks5(stream: &mut TcpStream) -> String {
    let mut greeting = [0_u8; 2];
    stream
        .read_exact(&mut greeting)
        .await
        .expect("SOCKS greeting");
    assert_eq!(greeting[0], 5);
    let mut methods = vec![0_u8; usize::from(greeting[1])];
    stream
        .read_exact(&mut methods)
        .await
        .expect("SOCKS methods");
    assert!(methods.contains(&0));
    stream
        .write_all(&[5, 0])
        .await
        .expect("SOCKS method response");

    let mut request = [0_u8; 4];
    stream
        .read_exact(&mut request)
        .await
        .expect("SOCKS request");
    assert_eq!(&request[..3], &[5, 1, 0]);
    let host = match request[3] {
        1 => {
            let mut address = [0_u8; 4];
            stream.read_exact(&mut address).await.expect("SOCKS IPv4");
            std::net::Ipv4Addr::from(address).to_string()
        }
        3 => {
            let length = stream.read_u8().await.expect("SOCKS domain length");
            let mut domain = vec![0_u8; usize::from(length)];
            stream.read_exact(&mut domain).await.expect("SOCKS domain");
            String::from_utf8(domain).expect("SOCKS domain UTF-8")
        }
        4 => {
            let mut address = [0_u8; 16];
            stream.read_exact(&mut address).await.expect("SOCKS IPv6");
            std::net::Ipv6Addr::from(address).to_string()
        }
        other => panic!("unexpected SOCKS address type {other}"),
    };
    let port = stream.read_u16().await.expect("SOCKS port");
    stream
        .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
        .await
        .expect("SOCKS connect response");
    format!("{host}:{port}")
}

async fn read_http_head(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.expect("HTTP request read");
        assert!(read > 0, "HTTP request ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        assert!(bytes.len() <= 64 * 1024, "HTTP request headers too large");
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).expect("HTTP request UTF-8")
}

async fn write_http_response(
    stream: &mut TcpStream,
    status: StatusCode,
    headers: &HeaderMap,
    body: &str,
) {
    let reason = status.canonical_reason().unwrap_or("Unknown");
    let mut response = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n",
        status.as_u16(),
        body.len()
    );
    for (name, value) in headers {
        response.push_str(name.as_str());
        response.push_str(": ");
        response.push_str(value.to_str().expect("response header UTF-8"));
        response.push_str("\r\n");
    }
    response.push_str("\r\n");
    response.push_str(body);
    stream
        .write_all(response.as_bytes())
        .await
        .expect("HTTP response write");
}
