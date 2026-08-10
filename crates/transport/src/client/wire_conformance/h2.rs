use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use bytes::Bytes;
use http::{Response, StatusCode};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpListener,
    sync::oneshot,
};
use tokio_rustls::TlsAcceptor;

use crate::{
    ReqwestTransportManager,
    api::{TransportManager, TransportManagerConfig, TransportProxy},
    connection::tests::TestTlsIdentity,
};

use super::support::{assert_fixture, fixture_request};

const FIXTURE: &str =
    include_str!("../../../testdata/generic-rustls-hyper-v2/http2-initial-frames.txt");
const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

#[tokio::test]
async fn http2_preface_and_initial_control_frames_match_the_versioned_wire_fixture() {
    let identity = TestTlsIdentity::generate();
    let (address, captured) = spawn_http2_capture(identity.server_config).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = any2api_domain::ProxyProfile::direct();
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            fixture_request(&format!(
                "https://localhost:{}/v1/responses",
                address.port()
            )),
        )
        .await
        .expect("HTTP/2 fixture response");
    assert_eq!(response.status, StatusCode::OK);
    let raw = tokio::time::timeout(Duration::from_secs(2), captured)
        .await
        .expect("HTTP/2 capture timeout")
        .expect("HTTP/2 capture");

    assert_fixture(&describe_initial_frames(&raw), FIXTURE);
}

async fn spawn_http2_capture(
    server_config: Arc<rustls::ServerConfig>,
) -> (std::net::SocketAddr, oneshot::Receiver<Vec<u8>>) {
    let mut server_config = Arc::try_unwrap(server_config)
        .unwrap_or_else(|_| panic!("fixture TLS config must be uniquely owned"));
    server_config.alpn_protocols = vec![b"h2".to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP/2 fixture listener");
    let address = listener.local_addr().expect("HTTP/2 fixture address");
    let (capture_tx, capture_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("HTTP/2 fixture connection");
        let stream = acceptor
            .accept(stream)
            .await
            .expect("fixture TLS handshake");
        let captured = Arc::new(Mutex::new(Vec::new()));
        let recording = RecordingIo::new(stream, Arc::clone(&captured));
        let mut connection = h2::server::handshake(recording)
            .await
            .expect("HTTP/2 fixture handshake");
        let (request, mut respond) = connection
            .accept()
            .await
            .expect("HTTP/2 fixture request")
            .expect("valid HTTP/2 fixture request");
        assert_eq!(request.uri().path(), "/v1/responses");
        let response = Response::builder()
            .status(StatusCode::OK)
            .body(())
            .expect("HTTP/2 response");
        let mut body = respond
            .send_response(response, false)
            .expect("HTTP/2 response headers");
        body.send_data(Bytes::from_static(b"ok"), true)
            .expect("HTTP/2 response body");
        let _ = tokio::time::timeout(Duration::from_millis(200), connection.accept()).await;
        let bytes = captured.lock().expect("HTTP/2 captured bytes").clone();
        capture_tx.send(bytes).expect("HTTP/2 capture receiver");
    });
    (address, capture_rx)
}

struct RecordingIo<T> {
    inner: T,
    captured: Arc<Mutex<Vec<u8>>>,
}

impl<T> RecordingIo<T> {
    fn new(inner: T, captured: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { inner, captured }
    }
}

impl<T: AsyncRead + Unpin> AsyncRead for RecordingIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(context, buffer);
        if result.is_ready() {
            self.captured
                .lock()
                .expect("HTTP/2 capture lock")
                .extend_from_slice(&buffer.filled()[before..]);
        }
        result
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for RecordingIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(context, buffer)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}

fn describe_initial_frames(bytes: &[u8]) -> String {
    assert!(bytes.starts_with(PREFACE), "HTTP/2 client preface");
    let mut output = String::from("preface=PRI * HTTP/2.0\\r\\n\\r\\nSM\\r\\n\\r\\n\n");
    let mut offset = PREFACE.len();
    while offset + 9 <= bytes.len() {
        let length = usize::from(bytes[offset]) << 16
            | usize::from(bytes[offset + 1]) << 8
            | usize::from(bytes[offset + 2]);
        let frame_type = bytes[offset + 3];
        let flags = bytes[offset + 4];
        let stream_id = u32::from_be_bytes([
            bytes[offset + 5] & 0x7f,
            bytes[offset + 6],
            bytes[offset + 7],
            bytes[offset + 8],
        ]);
        let payload_start = offset + 9;
        let payload_end = payload_start + length;
        assert!(payload_end <= bytes.len(), "complete HTTP/2 frame");
        if frame_type == 0x1 {
            output.push_str(&format!(
                "next=HEADERS flags=0x{flags:02x} stream={stream_id} length={length}\n"
            ));
            break;
        }
        output.push_str(&format!(
            "frame={} flags=0x{flags:02x} stream={stream_id} length={length}\n",
            frame_name(frame_type)
        ));
        let payload = &bytes[payload_start..payload_end];
        match frame_type {
            0x4 => describe_settings(payload, &mut output),
            0x8 if payload.len() == 4 => {
                let increment =
                    u32::from_be_bytes(payload.try_into().expect("window update")) & 0x7fff_ffff;
                output.push_str(&format!("  increment={increment}\n"));
            }
            _ => {}
        }
        offset = payload_end;
    }
    output
}

fn describe_settings(payload: &[u8], output: &mut String) {
    assert_eq!(payload.len() % 6, 0, "SETTINGS entry size");
    for setting in payload.chunks_exact(6) {
        let id = u16::from_be_bytes([setting[0], setting[1]]);
        let value = u32::from_be_bytes([setting[2], setting[3], setting[4], setting[5]]);
        output.push_str(&format!("  {}={value}\n", setting_name(id)));
    }
}

const fn frame_name(frame_type: u8) -> &'static str {
    match frame_type {
        0x0 => "DATA",
        0x1 => "HEADERS",
        0x2 => "PRIORITY",
        0x3 => "RST_STREAM",
        0x4 => "SETTINGS",
        0x6 => "PING",
        0x7 => "GOAWAY",
        0x8 => "WINDOW_UPDATE",
        0x9 => "CONTINUATION",
        _ => "UNKNOWN",
    }
}

const fn setting_name(id: u16) -> &'static str {
    match id {
        0x1 => "header_table_size",
        0x2 => "enable_push",
        0x3 => "max_concurrent_streams",
        0x4 => "initial_window_size",
        0x5 => "max_frame_size",
        0x6 => "max_header_list_size",
        _ => "unknown_setting",
    }
}
