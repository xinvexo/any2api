use std::time::Duration;

use any2api_domain::{
    ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId, RetrySafety,
};
use any2api_transport::api::{
    EndpointNetworkPolicy, ReqwestTransportManager, TransportFailureScope, TransportManager,
    TransportManagerConfig, TransportProxy, TransportRequest,
};
use axum::http::{HeaderMap, Method, Uri};
use bytes::Bytes;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

#[tokio::test]
async fn explicit_proxy_failure_never_falls_back_to_direct() {
    let origin = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("origin listener");
    let origin_address = origin.local_addr().expect("origin address");
    let (proxy_address, connect_request) = spawn_rejecting_connect_proxy().await;
    let proxy = ProxyProfile::create(
        ProxyProfileId::new(),
        ProxyDraft::new(
            "Unavailable",
            ProxyKind::Http,
            ProxyAddress::new(proxy_address.ip().to_string(), proxy_address.port())
                .expect("proxy address"),
            true,
        )
        .expect("proxy draft"),
    )
    .expect("proxy profile");
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        connect_timeout: Duration::from_millis(500),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let request = TransportRequest {
        method: Method::POST,
        uri: Uri::try_from(format!("https://{origin_address}/responses")).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::from_static(b"{}"),
        network_policy: EndpointNetworkPolicy::default(),
        read_timeout: Duration::from_secs(15),
    };

    let error = match manager
        .execute(TransportProxy::new(&proxy, None), request)
        .await
    {
        Ok(_) => panic!("explicit proxy failure must not use the origin directly"),
        Err(error) => error,
    };

    assert_eq!(
        error.stage,
        any2api_transport::api::TransportErrorStage::ProxyHandshake
    );
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
