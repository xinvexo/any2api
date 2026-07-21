use std::time::Duration;

use any2api_domain::{ProxyProfile, RetrySafety};
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
        Duration::from_millis(500),
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

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::POST,
        uri: Uri::try_from(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::from_static(b"{}"),
        network_policy: EndpointNetworkPolicy::new(true),
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
