use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    str::FromStr,
    time::Duration,
};

use any2api_domain::{ProxyKind, RetrySafety};
use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
};

use crate::{
    api::{
        EndpointNetworkPolicy, ProxyCredentials, TransportManager, TransportManagerConfig,
        TransportProxy, TransportRequest,
    },
    client::ReqwestTransportManager,
    client::tests as plain,
    connection::tests as connect,
    error::{TransportErrorStage, TransportFailureScope},
    resolution::shared_dns_cache,
};

#[tokio::test]
async fn strict_http_proxy_tunnels_to_a_pinned_ip_and_preserves_auth_and_host() {
    let (origin_address, origin_request) =
        plain::spawn_http_response(StatusCode::OK, HeaderMap::new(), "strict-proxy").await;
    let (proxy_address, connect_request) = connect::spawn_connect_proxy(origin_address).await;
    let manager = ReqwestTransportManager::default();
    let proxy = connect::network_proxy(proxy_address)
        .set_authentication("proxy-user")
        .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "proxy-password".to_owned());
    let response = manager
        .execute(
            TransportProxy::new(&proxy, Some(&credentials)),
            strict_request_to(&format!(
                "http://localhost:{}/v1/test?mode=strict",
                origin_address.port()
            )),
        )
        .await
        .expect("strict HTTP proxy response");

    assert_eq!(
        plain::collect_body(response).await,
        Bytes::from_static(b"strict-proxy")
    );
    let connect = connect_request.await.expect("captured CONNECT request");
    assert!(connect.starts_with("CONNECT "));
    assert!(!connect.starts_with("CONNECT localhost"));
    assert!(connect.contains("Proxy-Authorization: Basic"));
    let request = origin_request.await.expect("captured origin request");
    assert!(request.starts_with("GET /v1/test?mode=strict HTTP/1.1"));
    assert!(!request.contains("Proxy-Authorization"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("host: localhost:{}", origin_address.port()))
    );
}

#[tokio::test]
async fn strict_http_proxy_falls_back_before_sending_the_upstream_request() {
    const HOST: &str = "strict-http-fallback.invalid";
    const REQUEST_BODY: &[u8] = b"request-body-secret-sentinel";

    let (origin_address, origin_request) = spawn_http_origin().await;
    shared_dns_cache().seed(
        HOST,
        vec![
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ],
    );
    let (proxy_address, connect_requests) = spawn_address_routing_connect_proxy().await;
    let manager = ReqwestTransportManager::default();
    let proxy = connect::network_proxy(proxy_address)
        .set_authentication("proxy-user")
        .expect("proxy authentication metadata");
    let credentials = ProxyCredentials::new("proxy-user".to_owned(), "proxy-password".to_owned());
    let mut request = strict_request_to(&format!(
        "http://{HOST}:{}/v1/fallback",
        origin_address.port()
    ));
    request.method = Method::POST;
    request.body = Bytes::from_static(REQUEST_BODY);

    let response = manager
        .execute(TransportProxy::new(&proxy, Some(&credentials)), request)
        .await
        .expect("second pinned address must be tried before sending the request");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        plain::collect_body(response).await,
        Bytes::from_static(b"fallback-ok")
    );
    let connects = connect_requests.await.expect("captured CONNECT attempts");
    assert_eq!(connects.len(), 2);
    assert!(connects.iter().any(|request| request.starts_with(&format!(
        "CONNECT 127.0.0.2:{} HTTP/1.1",
        origin_address.port()
    ))));
    assert!(connects.iter().any(|request| request.starts_with(&format!(
        "CONNECT 127.0.0.1:{} HTTP/1.1",
        origin_address.port()
    ))));
    assert!(
        connects
            .iter()
            .all(|request| request.contains("Proxy-Authorization: Basic"))
    );
    assert!(connects.iter().all(|request| {
        !request
            .as_bytes()
            .windows(REQUEST_BODY.len())
            .any(|window| window == REQUEST_BODY)
    }));

    let request = String::from_utf8(origin_request.await.expect("captured origin request"))
        .expect("origin request UTF-8");
    assert!(request.starts_with("POST /v1/fallback HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains(&format!("host: {HOST}:{}", origin_address.port()))
    );
    assert!(!request.contains("Proxy-Authorization"));
    assert!(request.as_bytes().ends_with(REQUEST_BODY));
}

#[tokio::test]
async fn strict_socks5_uses_an_ip_target_and_preserves_host() {
    let (proxy_address, target, request) = plain::spawn_socks5_response("strict-socks").await;
    let manager = ReqwestTransportManager::default();
    let proxy = plain::network_proxy("SOCKS5", ProxyKind::Socks5, proxy_address, true);
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to("http://localhost:80/socks"),
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
            strict_request_to(&format!(
                "https://localhost:{}/strict-tunnel",
                origin_address.port()
            )),
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
async fn strict_connect_attributes_endpoint_tls_failure_to_the_egress_path() {
    let identity = connect::TestTlsIdentity::generate();
    let origin_address = connect::spawn_tls_handshake_endpoint(identity.server_config).await;
    let (proxy_address, _connect_request) = connect::spawn_connect_proxy(origin_address).await;
    let manager = ReqwestTransportManager::default();
    let proxy = connect::network_proxy(proxy_address);

    let error = match manager
        .execute(
            TransportProxy::new(&proxy, None),
            strict_request_to(&format!(
                "https://localhost:{}/untrusted-certificate",
                origin_address.port()
            )),
        )
        .await
    {
        Ok(_) => panic!("untrusted endpoint certificate must fail"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::Tls);
    assert_eq!(error.failure_scope, TransportFailureScope::EgressPath);
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
            strict_request_to(&format!(
                "https://localhost:{}/rejected",
                origin_address.port()
            )),
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

fn strict_request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        isolation: crate::client::tests::test_isolation(),
        network_policy: EndpointNetworkPolicy::new().with_strict_ssrf(true),
        read_timeout: Duration::from_secs(15),
    }
}

async fn spawn_http_origin() -> (SocketAddr, oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("origin listener");
    let address = listener.local_addr().expect("origin address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("origin connection");
        let request = read_http_request(&mut stream).await;
        request_tx.send(request).ok();
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nfallback-ok",
            )
            .await
            .expect("origin response");
    });
    (address, request_rx)
}

async fn spawn_address_routing_connect_proxy() -> (SocketAddr, oneshot::Receiver<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy listener");
    let address = listener.local_addr().expect("proxy address");
    let (requests_tx, requests_rx) = oneshot::channel();
    let (captured_tx, mut captured_rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        for _ in 0..2 {
            let (mut downstream, _) = listener.accept().await.expect("proxy connection");
            let captured_tx = captured_tx.clone();
            tokio::spawn(async move {
                let request = String::from_utf8(read_http_head(&mut downstream).await)
                    .expect("CONNECT request UTF-8");
                let target = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .and_then(|authority| SocketAddr::from_str(authority).ok())
                    .expect("CONNECT socket address");
                captured_tx.send(request).ok();
                if target.ip() == IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)) {
                    let mut closed = [0_u8; 1];
                    let _ = downstream.read(&mut closed).await;
                    return;
                }
                let mut upstream = TcpStream::connect(target)
                    .await
                    .expect("live origin connection");
                downstream
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .await
                    .expect("CONNECT response");
                let _ = copy_bidirectional(&mut downstream, &mut upstream).await;
            });
        }
        drop(captured_tx);
        let mut requests = Vec::new();
        while let Some(request) = captured_rx.recv().await {
            requests.push(request);
        }
        requests_tx.send(requests).ok();
    });
    (address, requests_rx)
}

async fn read_http_head(stream: &mut TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.expect("HTTP head read");
        assert!(read > 0, "HTTP connection ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        assert!(bytes.len() <= 64 * 1024, "HTTP headers too large");
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            return bytes;
        }
    }
}

async fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut bytes = read_http_head(stream).await;
    let head_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP header terminator")
        + 4;
    let head = std::str::from_utf8(&bytes[..head_end]).expect("HTTP head UTF-8");
    let content_length = head
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().expect("Content-Length"))
        })
        .unwrap_or(0);
    let captured_body = bytes.len().saturating_sub(head_end);
    bytes.resize(head_end + content_length, 0);
    if captured_body < content_length {
        stream
            .read_exact(&mut bytes[head_end + captured_body..])
            .await
            .expect("HTTP request body");
    }
    bytes
}
