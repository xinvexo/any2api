use std::time::Duration;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};

use crate::{
    ReqwestTransportManager,
    api::{TransportManager, TransportProxy},
};

use super::support::{assert_fixture, fixture_request};

const FIXTURE: &str =
    include_str!("../../../testdata/generic-rustls-hyper-v3/http1-request-head.txt");

#[tokio::test]
async fn http1_raw_request_head_matches_the_versioned_wire_fixture() {
    let (address, captured) = spawn_raw_http1_server().await;
    let manager = ReqwestTransportManager::default();
    let proxy = any2api_domain::ProxyProfile::direct();
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            fixture_request(&format!("http://{address}/v1/responses?fixture=h1")),
        )
        .await
        .expect("HTTP/1 fixture response");
    assert_eq!(response.status, http::StatusCode::OK);
    let raw = tokio::time::timeout(Duration::from_secs(2), captured)
        .await
        .expect("HTTP/1 capture timeout")
        .expect("HTTP/1 capture");
    let normalized = String::from_utf8(raw)
        .expect("HTTP/1 request head is ASCII")
        .replace(&address.to_string(), "<authority>")
        .replace("\r\n", "\\r\\n\n");

    assert_fixture(&normalized, FIXTURE);
}

async fn spawn_raw_http1_server() -> (std::net::SocketAddr, oneshot::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP/1 fixture listener");
    let address = listener.local_addr().expect("HTTP/1 fixture address");
    let (capture_tx, capture_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("HTTP/1 fixture connection");
        let mut captured = Vec::new();
        let mut buffer = [0_u8; 512];
        while !captured.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream
                .read(&mut buffer)
                .await
                .expect("HTTP/1 request bytes");
            assert!(
                read > 0,
                "HTTP/1 request ended before its header terminator"
            );
            captured.extend_from_slice(&buffer[..read]);
        }
        let head_end = captured
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("HTTP/1 header terminator")
            + 4;
        captured.truncate(head_end);
        capture_tx.send(captured).expect("HTTP/1 capture receiver");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
            .await
            .expect("HTTP/1 fixture response");
    });
    (address, capture_rx)
}
