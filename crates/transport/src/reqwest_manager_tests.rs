use std::{str::FromStr, sync::Arc, time::Duration};

use any2api_domain::{
    ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId, RetrySafety,
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
    api::{TransportManager, TransportManagerConfig, TransportProxy, TransportRequest},
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

    let error = match manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to(&format!("https://{origin_address}/must-not-connect")),
        )
        .await
    {
        Ok(_) => panic!("explicit proxy failure must not use DIRECT"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::Unattributed);
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
    let first = manager.client_for(&original).expect("first client");
    let reused = manager.client_for(&original).expect("reused client");
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
        .client_for(&updated)
        .expect("next generation client");
    assert!(!Arc::ptr_eq(&first, &next));
    assert_eq!(manager.cached_client_count(), 1);

    let rebuilt = manager.client_for(&original).expect("rebuilt old client");
    assert!(!Arc::ptr_eq(&first, &rebuilt));
    assert_eq!(manager.cached_client_count(), 1);
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

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

pub(super) fn network_proxy(
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

pub(super) async fn collect_body(mut response: crate::api::TransportResponse) -> Bytes {
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next().await {
        body.extend_from_slice(&chunk.expect("response body chunk"));
    }
    body.into()
}

pub(super) async fn spawn_http_response(
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

pub(super) async fn spawn_socks5_response(
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
