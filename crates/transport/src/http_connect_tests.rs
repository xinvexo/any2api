use std::{str::FromStr, sync::Arc};

use any2api_domain::{ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId};
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, Method, StatusCode, Uri};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use rustls::{ServerConfig, pki_types::PrivatePkcs8KeyDer};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_rustls::TlsAcceptor;

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, TransportManager, TransportManagerConfig, TransportRequest,
        TransportResponse,
    },
};

#[tokio::test]
async fn https_upstream_uses_an_http_connect_tunnel() {
    let identity = TestTlsIdentity::generate();
    let (origin_address, origin_request) =
        spawn_https_response(identity.server_config, "tunneled").await;
    let (proxy_address, connect_request) = spawn_connect_proxy(origin_address).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = network_proxy(proxy_address);

    let response = manager
        .execute(
            &proxy,
            request_to(&format!(
                "https://localhost:{}/through-proxy",
                origin_address.port()
            )),
        )
        .await
        .expect("HTTPS response through HTTP proxy");

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        collect_body(response).await,
        Bytes::from_static(b"tunneled")
    );
    assert!(
        connect_request
            .await
            .expect("captured CONNECT request")
            .starts_with(&format!(
                "CONNECT localhost:{} HTTP/1.1",
                origin_address.port()
            ))
    );
    assert!(
        origin_request
            .await
            .expect("captured origin request")
            .starts_with("GET /through-proxy HTTP/1.1")
    );
}

struct TestTlsIdentity {
    client_certificate: reqwest::Certificate,
    server_config: Arc<ServerConfig>,
}

impl TestTlsIdentity {
    fn generate() -> Self {
        let CertifiedKey { cert, key_pair } =
            generate_simple_self_signed(vec!["localhost".to_owned()])
                .expect("self-signed certificate");
        let certificate_der = cert.der().clone();
        let client_certificate = reqwest::Certificate::from_der(certificate_der.as_ref())
            .expect("reqwest root certificate");
        let private_key = PrivatePkcs8KeyDer::from(key_pair.serialize_der());
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certificate_der], private_key.into())
            .expect("TLS server config");
        Self {
            client_certificate,
            server_config: Arc::new(server_config),
        }
    }
}

async fn spawn_https_response(
    server_config: Arc<ServerConfig>,
    body: &'static str,
) -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTPS listener");
    let address = listener.local_addr().expect("HTTPS address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("HTTPS connection");
        let mut stream = TlsAcceptor::from(server_config)
            .accept(stream)
            .await
            .expect("TLS handshake");
        let request = read_http_head(&mut stream).await;
        request_tx.send(request).ok();
        write_http_response(&mut stream, body).await;
    });
    (address, request_rx)
}

async fn spawn_connect_proxy(
    origin_address: std::net::SocketAddr,
) -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP proxy listener");
    let address = listener.local_addr().expect("HTTP proxy address");
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut downstream, _) = listener.accept().await.expect("proxy connection");
        let request = read_http_head(&mut downstream).await;
        request_tx.send(request).ok();
        let mut upstream = TcpStream::connect(origin_address)
            .await
            .expect("origin connection");
        downstream
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await
            .expect("CONNECT response");
        copy_bidirectional(&mut downstream, &mut upstream)
            .await
            .expect("CONNECT tunnel");
    });
    (address, request_rx)
}

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        network_policy: EndpointNetworkPolicy::new(true),
    }
}

fn network_proxy(address: std::net::SocketAddr) -> ProxyProfile {
    let address =
        ProxyAddress::new(address.ip().to_string(), address.port()).expect("proxy address");
    let draft =
        ProxyDraft::new("HTTP CONNECT", ProxyKind::Http, address, true).expect("proxy draft");
    ProxyProfile::create(ProxyProfileId::new(), draft).expect("proxy profile")
}

async fn collect_body(mut response: TransportResponse) -> Bytes {
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next().await {
        body.extend_from_slice(&chunk.expect("response body chunk"));
    }
    body.into()
}

async fn read_http_head<S>(stream: &mut S) -> String
where
    S: AsyncRead + Unpin,
{
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

async fn write_http_response<S>(stream: &mut S, body: &str)
where
    S: AsyncWrite + Unpin,
{
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("HTTP response write");
}
