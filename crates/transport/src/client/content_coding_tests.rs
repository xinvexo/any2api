use std::{str::FromStr, time::Duration};

use any2api_domain::{ProxyKind, ProxyProfile};
use async_compression::tokio::write::GzipEncoder;
use bytes::Bytes;
use futures_util::StreamExt;
use http::{HeaderMap, Method, StatusCode, Uri, header};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};

use super::{ReqwestTransportManager, tests::network_proxy};
use crate::{
    api::{
        EndpointNetworkPolicy, TransportManager, TransportProxy, TransportRequest,
        TransportTrafficClass,
    },
    isolation::TransportIsolationKey,
};

const PAYLOAD: &[u8] = b"{\"object\":\"response\",\"output\":[]}";

#[tokio::test]
async fn direct_client_negotiates_and_decodes_gzip_success() {
    let (address, captured) = spawn_gzip_server(StatusCode::OK).await;
    let proxy = ProxyProfile::direct();
    assert_gzip_exchange(
        &proxy,
        &format!("http://{address}/direct"),
        EndpointNetworkPolicy::new(),
        StatusCode::OK,
        captured,
    )
    .await;
}

#[tokio::test]
async fn direct_client_decodes_error_content_before_classification() {
    let (address, captured) = spawn_gzip_server(StatusCode::BAD_REQUEST).await;
    let proxy = ProxyProfile::direct();
    assert_gzip_exchange(
        &proxy,
        &format!("http://{address}/error"),
        EndpointNetworkPolicy::new(),
        StatusCode::BAD_REQUEST,
        captured,
    )
    .await;
}

#[tokio::test]
async fn pinned_proxy_client_uses_the_same_response_coding_boundary() {
    let (proxy_address, captured) = spawn_gzip_server(StatusCode::OK).await;
    let proxy = network_proxy("strict HTTP", ProxyKind::Http, proxy_address, true);
    assert_gzip_exchange(
        &proxy,
        "http://127.0.0.1:9/pinned",
        EndpointNetworkPolicy::new().with_strict_ssrf(true),
        StatusCode::OK,
        captured,
    )
    .await;
}

async fn assert_gzip_exchange(
    proxy: &ProxyProfile,
    uri: &str,
    network_policy: EndpointNetworkPolicy,
    expected_status: StatusCode,
    captured: oneshot::Receiver<String>,
) {
    let manager = ReqwestTransportManager::default();
    let response = manager
        .execute(
            TransportProxy::new(proxy, None),
            TransportRequest {
                method: Method::GET,
                uri: Uri::from_str(uri).expect("request URI"),
                headers: HeaderMap::new(),
                body: Bytes::new(),
                isolation: TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic),
                network_policy,
                read_timeout: Duration::from_secs(5),
            },
        )
        .await
        .expect("compressed response");

    assert_eq!(response.status, expected_status);
    assert!(!response.headers.contains_key(header::CONTENT_ENCODING));
    assert!(!response.headers.contains_key(header::CONTENT_LENGTH));
    assert_eq!(collect(response).await, PAYLOAD);
    let request = captured
        .await
        .expect("captured request")
        .to_ascii_lowercase();
    assert!(request.contains("\r\naccept-encoding: gzip, br, zstd\r\n"));
}

async fn spawn_gzip_server(
    status: StatusCode,
) -> (std::net::SocketAddr, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("gzip listener");
    let address = listener.local_addr().expect("gzip address");
    let encoded = encode_gzip().await;
    let (request_tx, request_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("gzip connection");
        request_tx.send(read_http_head(&mut stream).await).ok();
        let reason = status.canonical_reason().expect("standard fixture status");
        let head = format!(
            "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            status.as_u16(),
            encoded.len(),
        );
        stream
            .write_all(head.as_bytes())
            .await
            .expect("gzip response head");
        for byte in encoded {
            stream.write_all(&[byte]).await.expect("gzip response byte");
        }
    });
    (address, request_rx)
}

async fn encode_gzip() -> Vec<u8> {
    let mut encoder = GzipEncoder::new(Vec::new());
    encoder.write_all(PAYLOAD).await.expect("gzip write");
    encoder.shutdown().await.expect("gzip finish");
    encoder.into_inner()
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

async fn collect(mut response: crate::api::TransportResponse) -> Bytes {
    let mut body = Vec::new();
    while let Some(chunk) = response.body.next().await {
        body.extend_from_slice(&chunk.expect("decoded response body"));
    }
    body.into()
}
