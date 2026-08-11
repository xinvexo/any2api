use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http::{Response, StatusCode};
use tokio::{net::TcpListener, sync::oneshot};
use tokio_rustls::TlsAcceptor;

use crate::{
    ReqwestTransportManager,
    api::{
        TransportIsolationKey, TransportManager, TransportManagerConfig, TransportProxy,
        TransportRequest, TransportTrafficClass,
    },
    client::tests::collect_body,
    connection::tests::TestTlsIdentity,
};

use super::support::{assert_fixture, fixture_request};
use frames::{describe_client_lifecycle, describe_initial_frames, describe_server_control};
use probe::{H2Probe, ScenarioCapture, spawn_concurrent, spawn_goaway, spawn_sequential};
use recording::WireCapture;

mod frames;
mod probe;
mod recording;

const INITIAL_FIXTURE: &str =
    include_str!("../../../testdata/generic-rustls-hyper-v3/http2-initial-frames.txt");
const LIFECYCLE_FIXTURE: &str =
    include_str!("../../../testdata/generic-rustls-hyper-v3/http2-connection-lifecycle.txt");

#[tokio::test]
async fn http2_preface_and_initial_control_frames_match_the_versioned_wire_fixture() {
    let identity = TestTlsIdentity::generate();
    let (address, captured) = spawn_http2_capture(identity.server_config).await;
    let manager = manager(identity.client_certificate);
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

    assert_fixture(&describe_initial_frames(&raw), INITIAL_FIXTURE);
}

#[tokio::test]
async fn http2_connection_lifecycle_matches_the_versioned_wire_fixture() {
    let sequential = capture_sequential().await;
    let concurrent = capture_concurrent().await;
    let goaway = capture_goaway().await;
    let actual = [
        describe_scenario("sequential-reuse", &sequential),
        describe_scenario("concurrent-streams", &concurrent),
        describe_scenario("goaway-reconnect", &goaway),
    ]
    .join("\n");

    assert_fixture(&actual, LIFECYCLE_FIXTURE);
}

async fn capture_sequential() -> ScenarioCapture {
    let identity = TestTlsIdentity::generate();
    let probe = spawn_sequential(identity.server_config).await;
    let manager = manager(identity.client_certificate);
    let proxy = any2api_domain::ProxyProfile::direct();
    let isolation = TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic);

    execute_ok(&manager, &proxy, request(&probe, isolation)).await;
    execute_ok(&manager, &proxy, request(&probe, isolation)).await;
    probe.finish().await
}

async fn capture_concurrent() -> ScenarioCapture {
    let identity = TestTlsIdentity::generate();
    let mut probe = spawn_concurrent(identity.server_config).await;
    let manager = manager(identity.client_certificate);
    let proxy = any2api_domain::ProxyProfile::direct();
    let isolation = TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic);

    let first = manager.execute(
        TransportProxy::new(&proxy, None),
        request(&probe, isolation),
    );
    tokio::pin!(first);
    tokio::select! {
        () = probe.wait_for_first_request() => {}
        result = first.as_mut() => {
            match result {
                Ok(_) => panic!("first concurrent response arrived before the second stream"),
                Err(error) => panic!("first concurrent request failed before the gate: {error}"),
            }
        }
    }
    let second = manager.execute(
        TransportProxy::new(&proxy, None),
        request(&probe, isolation),
    );
    let (first, second) = tokio::join!(first.as_mut(), second);
    assert_ok(first.expect("first concurrent response")).await;
    assert_ok(second.expect("second concurrent response")).await;
    probe.finish().await
}

async fn capture_goaway() -> ScenarioCapture {
    let identity = TestTlsIdentity::generate();
    let probe = spawn_goaway(identity.server_config).await;
    let manager = manager(identity.client_certificate);
    let proxy = any2api_domain::ProxyProfile::direct();
    let isolation = TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic);

    execute_ok(&manager, &proxy, request(&probe, isolation)).await;
    probe.wait_for_first_goaway().await;
    execute_ok(&manager, &proxy, request(&probe, isolation)).await;
    probe.finish().await
}

fn manager(certificate: Vec<u8>) -> ReqwestTransportManager {
    ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        certificate,
    )
    .expect("transport manager")
}

fn request(probe: &H2Probe, isolation: TransportIsolationKey) -> TransportRequest {
    let mut request = fixture_request(&format!(
        "https://localhost:{}/v1/responses",
        probe.address.port()
    ));
    request.body = Bytes::new();
    request.isolation = isolation;
    request
}

async fn execute_ok(
    manager: &ReqwestTransportManager,
    proxy: &any2api_domain::ProxyProfile,
    request: TransportRequest,
) {
    let response = manager
        .execute(TransportProxy::new(proxy, None), request)
        .await
        .expect("HTTP/2 lifecycle response");
    assert_ok(response).await;
}

async fn assert_ok(response: crate::api::TransportResponse) {
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(collect_body(response).await, Bytes::from_static(b"ok"));
}

fn describe_scenario(name: &str, capture: &ScenarioCapture) -> String {
    let mut output = format!(
        "scenario={name}\nconnection_count={}\n",
        capture.connections.len()
    );
    for (index, connection) in capture.connections.iter().enumerate() {
        output.push_str(&format!("connection={}\n", index + 1));
        output.push_str(&describe_client_lifecycle(&connection.client_bytes()));
        output.push_str(&describe_server_control(&connection.server_bytes()));
    }
    for event in &capture.events {
        output.push_str(&format!("event={event}\n"));
    }
    output
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
    let capture = WireCapture::default();
    let task_capture = capture.clone();
    let (capture_tx, capture_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("HTTP/2 fixture connection");
        let stream = acceptor
            .accept(stream)
            .await
            .expect("fixture TLS handshake");
        let mut connection = h2::server::handshake(task_capture.recording(stream))
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
        capture_tx
            .send(task_capture.client_bytes())
            .expect("HTTP/2 capture receiver");
    });
    (address, capture_rx)
}
