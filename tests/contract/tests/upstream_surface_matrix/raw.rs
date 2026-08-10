use std::{str::FromStr, time::Duration};

use any2api_domain::ProxyProfile;
use any2api_transport::api::{ReqwestTransportManager, TransportManager, TransportProxy};
use futures_util::StreamExt;
use http::Uri;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

use super::types::SurfaceCase;

pub(super) async fn capture(
    manager: &ReqwestTransportManager,
    proxy: &ProxyProfile,
    mut case: SurfaceCase,
) -> String {
    let (address, captured) = raw_http1_server().await;
    let path = case
        .request
        .uri
        .path_and_query()
        .map_or("/", http::uri::PathAndQuery::as_str);
    case.request.uri =
        Uri::from_str(&format!("http://{address}{path}")).expect("loopback surface endpoint");
    let mut response = manager
        .execute(TransportProxy::new(proxy, None), case.request)
        .await
        .expect("surface loopback request");
    assert_eq!(response.status, http::StatusCode::OK);
    while let Some(chunk) = response.body.next().await {
        chunk.expect("surface response body");
    }
    let raw = tokio::time::timeout(Duration::from_secs(2), captured)
        .await
        .expect("surface capture timeout")
        .expect("surface capture");
    let raw = normalize_raw(raw, address);
    format!(
        "=== {} ===\nprovider: {}\nsurface: {}\nauth-class: {}\ntarget: {}\n{}",
        case.name,
        case.provider.as_str(),
        case.surface.as_str(),
        case.auth_class,
        case.target,
        raw.trim_end_matches('\n')
    )
}

async fn raw_http1_server() -> (std::net::SocketAddr, oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("surface recorder listener");
    let address = listener.local_addr().expect("surface recorder address");
    let (capture_tx, capture_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("surface connection");
        let mut captured = Vec::new();
        let mut buffer = [0_u8; 4096];
        let head_end = loop {
            if let Some(position) = captured.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
            let read = stream
                .read(&mut buffer)
                .await
                .expect("surface request head");
            assert!(read > 0, "surface request ended before its head");
            captured.extend_from_slice(&buffer[..read]);
        };
        let content_length = content_length(&captured[..head_end]);
        while captured.len() < head_end + content_length {
            let read = stream
                .read(&mut buffer)
                .await
                .expect("surface request body");
            assert!(read > 0, "surface request body ended early");
            captured.extend_from_slice(&buffer[..read]);
        }
        captured.truncate(head_end + content_length);
        capture_tx.send(captured).expect("surface capture receiver");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
            .await
            .expect("surface response");
    });
    (address, capture_rx)
}

fn content_length(head: &[u8]) -> usize {
    let head = std::str::from_utf8(head).expect("surface request head is ASCII");
    head.split("\r\n")
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(str::trim)
                .map(|value| value.parse().expect("surface Content-Length"))
        })
        .unwrap_or_default()
}

fn normalize_raw(raw: Vec<u8>, address: std::net::SocketAddr) -> String {
    let mut raw = String::from_utf8(raw).expect("surface request is UTF-8");
    if let Some(boundary) = multipart_boundary(&raw) {
        raw = raw.replace(&boundary, "<multipart-boundary>");
    }
    raw = raw.replace(&address.to_string(), "<authority>");
    raw = raw.replace(
        &format!(
            "grok-shell/0.2.112 ({}; {})",
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        "grok-shell/0.2.112 (<os>; <arch>)",
    );
    raw.replace("\r\n", "\\r\\n\n")
}

fn multipart_boundary(raw: &str) -> Option<String> {
    let marker = "boundary=any2api_";
    let start = raw.find(marker)? + "boundary=".len();
    let end = start + raw[start..].find("\r\n")?;
    Some(raw[start..end].to_owned())
}
