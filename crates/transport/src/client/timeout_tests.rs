use std::time::Duration;

use any2api_domain::{ProxyKind, ProxyProfile, RetrySafety};
use bytes::Bytes;
use http::{HeaderMap, Method, Uri};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::oneshot};

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, TransportManager, TransportManagerConfig, TransportProxy,
        TransportRequest,
    },
    error::{TransportErrorStage, TransportFailureScope},
};

use super::tests::network_proxy;

#[tokio::test]
async fn stalled_response_headers_use_the_request_read_timeout() {
    let (address, request) = spawn_stalled_response_headers().await;
    let manager = ReqwestTransportManager::default();
    let mut transport_request = request_to(&format!("http://{address}/stalled-headers"));
    transport_request.read_timeout = Duration::from_millis(25);

    let error = match manager
        .execute(
            TransportProxy::new(&ProxyProfile::direct(), None),
            transport_request,
        )
        .await
    {
        Ok(_) => panic!("stalled response headers must time out"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::AwaitHeaders);
    assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
    assert_eq!(error.retry_safety, RetrySafety::Ambiguous);
    assert!(
        request
            .await
            .expect("captured request")
            .starts_with("POST /stalled-headers HTTP/1.1")
    );
}

#[tokio::test]
async fn connect_timeout_is_not_replaced_by_a_short_read_timeout() {
    let address = spawn_stalled_tls_handshake().await;
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        connect_timeout: Duration::from_millis(50),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let mut transport_request =
        request_to(&format!("https://localhost:{}/stalled-tls", address.port()));
    transport_request.read_timeout = Duration::from_millis(1);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        manager.execute(
            TransportProxy::new(&ProxyProfile::direct(), None),
            transport_request,
        ),
    )
    .await
    .expect("connect timeout must finish");
    let error = match result {
        Ok(_) => panic!("stalled TLS handshake must fail"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::Tcp);
    assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
}

#[tokio::test]
async fn expired_deadline_after_client_build_never_starts_network_io() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("origin listener");
    let address = listener.local_addr().expect("origin address");
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        connect_timeout: Duration::from_nanos(1),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");

    let error = match manager
        .execute(
            TransportProxy::new(&ProxyProfile::direct(), None),
            request_to(&format!("http://{address}/must-not-connect")),
        )
        .await
    {
        Ok(_) => panic!("expired connect deadline must fail"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::Tcp);
    assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn stalled_http_connect_handshake_uses_the_absolute_connect_deadline() {
    let (proxy_address, connect_request) = spawn_stalled_http_connect().await;
    let manager = manager_with_connect_timeout();
    let proxy = network_proxy("stalled CONNECT", ProxyKind::Http, proxy_address, true);
    manager.client_for(&proxy).expect("warm proxy client");
    let started = tokio::time::Instant::now();

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        manager.execute(
            TransportProxy::new(&proxy, None),
            request_to("https://upstream.invalid/stalled-connect"),
        ),
    )
    .await
    .expect("connect deadline must finish");
    let error = match result {
        Ok(_) => panic!("stalled CONNECT handshake must fail"),
        Err(error) => error,
    };

    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::Unattributed);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
    assert!(
        connect_request
            .await
            .expect("captured CONNECT request")
            .starts_with("CONNECT upstream.invalid:443 HTTP/1.1")
    );
}

#[tokio::test]
async fn stalled_strict_socks5_handshake_uses_the_absolute_connect_deadline() {
    let (proxy_address, greeting, disconnected) = spawn_stalled_socks5_handshake().await;
    let manager = manager_with_connect_timeout();
    let proxy = network_proxy("stalled SOCKS5", ProxyKind::Socks5, proxy_address, true);
    let mut request = request_to("http://127.0.0.1:80/stalled-socks");
    request.network_policy = EndpointNetworkPolicy::new().with_strict_ssrf(true);
    manager
        .warm_client_for_request(TransportProxy::new(&proxy, None), &request)
        .await
        .expect("warm pinned proxy client");
    let started = tokio::time::Instant::now();

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        manager.execute(TransportProxy::new(&proxy, None), request),
    )
    .await
    .expect("connect deadline must finish");
    let error = match result {
        Ok(_) => panic!("stalled SOCKS5 handshake must fail"),
        Err(error) => error,
    };

    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::Unattributed);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
    tokio::time::timeout(Duration::from_secs(1), greeting)
        .await
        .expect("SOCKS5 greeting must arrive")
        .expect("captured SOCKS5 greeting");
    tokio::time::timeout(Duration::from_secs(1), disconnected)
        .await
        .expect("timed out connector must close the proxy connection")
        .expect("captured SOCKS5 disconnect");
}

fn manager_with_connect_timeout() -> ReqwestTransportManager {
    ReqwestTransportManager::new(TransportManagerConfig {
        connect_timeout: Duration::from_millis(250),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager")
}

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::POST,
        uri: Uri::try_from(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::from_static(b"{}"),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

async fn spawn_stalled_response_headers() -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP listener");
    let address = listener.local_addr().expect("HTTP address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("HTTP connection");
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        std::future::pending::<()>().await;
    });
    (address, request_rx)
}

async fn spawn_stalled_tls_handshake() -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("TLS listener");
    let address = listener.local_addr().expect("TLS address");
    tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.expect("TLS connection");
        std::future::pending::<()>().await;
    });
    address
}

async fn spawn_stalled_http_connect() -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP proxy listener");
    let address = listener.local_addr().expect("HTTP proxy address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("HTTP proxy connection");
        request_tx.send(read_http_head(&mut stream).await).ok();
        std::future::pending::<()>().await;
    });
    (address, request_rx)
}

async fn spawn_stalled_socks5_handshake() -> (
    std::net::SocketAddr,
    oneshot::Receiver<()>,
    oneshot::Receiver<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("SOCKS5 proxy listener");
    let address = listener.local_addr().expect("SOCKS5 proxy address");
    let (greeting_tx, greeting_rx) = oneshot::channel();
    let (disconnected_tx, disconnected_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("SOCKS5 proxy connection");
        let mut header = [0_u8; 2];
        stream
            .read_exact(&mut header)
            .await
            .expect("SOCKS5 greeting header");
        assert_eq!(header[0], 5);
        let mut methods = vec![0_u8; usize::from(header[1])];
        stream
            .read_exact(&mut methods)
            .await
            .expect("SOCKS5 greeting methods");
        greeting_tx.send(()).ok();
        let mut byte = [0_u8; 1];
        let _ = stream.read(&mut byte).await;
        disconnected_tx.send(()).ok();
    });
    (address, greeting_rx, disconnected_rx)
}

async fn read_http_head(stream: &mut tokio::net::TcpStream) -> String {
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
