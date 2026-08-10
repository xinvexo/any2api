use std::{
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::{CredentialId, ProxyProfile, RoutingCredentialId};
use bytes::Bytes;
use http::{HeaderMap, Method, Response, StatusCode, Uri, header::AUTHORIZATION};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tokio_rustls::TlsAcceptor;

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, TransportIsolationKey, TransportManager, TransportManagerConfig,
        TransportProxy, TransportRequest, TransportTrafficClass,
    },
    connection::tests::TestTlsIdentity,
};

use super::tests::collect_body;

#[tokio::test]
async fn distinct_credentials_use_distinct_http2_connections() {
    let identity = TestTlsIdentity::generate();
    let mut probe = spawn_http2_probe(identity.server_config).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = ProxyProfile::direct();
    let uri = format!("https://localhost:{}/v1/responses", probe.address.port());
    let first_isolation =
        credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane);
    let second_isolation =
        credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane);

    let first_client = manager
        .client_for(&proxy, first_isolation)
        .expect("first cached client");
    let first = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-a", first_isolation),
        )
        .await
        .expect("first HTTP/2 response");
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(collect_body(first).await, Bytes::from_static(b"ok"));

    let second_client = manager
        .client_for(&proxy, second_isolation)
        .expect("second cached client");
    let second = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-b", second_isolation),
        )
        .await
        .expect("second HTTP/2 response");
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(collect_body(second).await, Bytes::from_static(b"ok"));

    assert!(!Arc::ptr_eq(&first_client, &second_client));
    let first_authorization = receive_observation(&mut probe.authorizations).await;
    let second_authorization = receive_observation(&mut probe.authorizations).await;
    assert_eq!(first_authorization, "Bearer credential-a");
    assert_eq!(second_authorization, "Bearer credential-b");
    assert_eq!(probe.accepted_connections.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn same_credential_generation_and_class_reuses_one_http2_connection() {
    let identity = TestTlsIdentity::generate();
    let mut probe = spawn_http2_probe(identity.server_config).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = ProxyProfile::direct();
    let uri = format!("https://localhost:{}/v1/responses", probe.address.port());
    let isolation = credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane);

    let first_client = manager
        .client_for(&proxy, isolation)
        .expect("first cached client");
    let first = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-a", isolation),
        )
        .await
        .expect("first HTTP/2 response");
    assert_eq!(collect_body(first).await, Bytes::from_static(b"ok"));
    let second_client = manager
        .client_for(&proxy, isolation)
        .expect("second cached client");
    let second = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-a", isolation),
        )
        .await
        .expect("second HTTP/2 response");
    assert_eq!(collect_body(second).await, Bytes::from_static(b"ok"));

    assert!(Arc::ptr_eq(&first_client, &second_client));
    assert_eq!(probe.accepted_connections.load(Ordering::SeqCst), 1);
    assert_eq!(
        receive_observation(&mut probe.authorizations).await,
        "Bearer credential-a"
    );
    assert_eq!(
        receive_observation(&mut probe.authorizations).await,
        "Bearer credential-a"
    );
    assert_eq!(
        receive_observation(&mut probe.handshakes).await,
        rustls::HandshakeKind::Full
    );
}

#[tokio::test]
async fn distinct_traffic_classes_do_not_share_tls_resumption_state() {
    let identity = TestTlsIdentity::generate();
    let mut probe = spawn_http2_probe(identity.server_config).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = ProxyProfile::direct();
    let uri = format!("https://localhost:{}/v1/responses", probe.address.port());
    let credential_id = CredentialId::new();
    let data_isolation = credential_isolation(credential_id, TransportTrafficClass::DataPlane);
    let quota_isolation = credential_isolation(credential_id, TransportTrafficClass::OAuthQuota);

    let data_client = manager
        .client_for(&proxy, data_isolation)
        .expect("data-plane client");
    let data = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-a", data_isolation),
        )
        .await
        .expect("data-plane response");
    assert_eq!(collect_body(data).await, Bytes::from_static(b"ok"));
    assert_eq!(
        receive_observation(&mut probe.handshakes).await,
        rustls::HandshakeKind::Full
    );

    let quota_client = manager
        .client_for(&proxy, quota_isolation)
        .expect("quota client");
    let quota = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-a", quota_isolation),
        )
        .await
        .expect("quota response");
    assert_eq!(collect_body(quota).await, Bytes::from_static(b"ok"));

    assert!(!Arc::ptr_eq(&data_client, &quota_client));
    assert_eq!(probe.accepted_connections.load(Ordering::SeqCst), 2);
    assert_eq!(
        receive_observation(&mut probe.handshakes).await,
        rustls::HandshakeKind::Full
    );
}

#[test]
fn inactive_credential_clients_are_evicted_by_the_bounded_lru() {
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        max_cached_clients: 2,
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let proxy = ProxyProfile::direct();
    let first = manager
        .client_for(
            &proxy,
            credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane),
        )
        .expect("first credential client");
    let second = manager
        .client_for(
            &proxy,
            credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane),
        )
        .expect("second credential client");
    let third = manager
        .client_for(
            &proxy,
            credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane),
        )
        .expect("third credential client");

    assert_eq!(manager.cached_client_count(), 2);
    assert_eq!(Arc::strong_count(&first), 1);
    assert_eq!(Arc::strong_count(&second), 2);
    assert_eq!(Arc::strong_count(&third), 2);
}

#[tokio::test]
async fn inactive_keepalive_connection_closes_at_the_pool_idle_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP/1 idle listener");
    let address = listener.local_addr().expect("HTTP/1 idle address");
    let (closed_tx, closed_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("HTTP/1 idle connection");
        let mut request_head = Vec::new();
        let mut buffer = [0_u8; 512];
        while !request_head.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).await.expect("HTTP/1 request head");
            assert!(read > 0, "request ended before the header terminator");
            request_head.extend_from_slice(&buffer[..read]);
        }
        stream
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok")
            .await
            .expect("HTTP/1 idle response");
        let mut byte = [0_u8; 1];
        let read = stream.read(&mut byte).await.expect("HTTP/1 idle close");
        assert_eq!(read, 0, "idle pooled connection remained open");
        closed_tx.send(()).expect("idle close receiver");
    });

    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        pool_idle_timeout: Duration::from_millis(50),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let proxy = ProxyProfile::direct();
    let isolation = credential_isolation(CredentialId::new(), TransportTrafficClass::DataPlane);
    let response = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_without_body(&format!("http://{address}/idle"), isolation),
        )
        .await
        .expect("HTTP/1 idle response");
    assert_eq!(collect_body(response).await, Bytes::from_static(b"ok"));
    assert_eq!(manager.cached_client_count(), 1);
    tokio::time::timeout(Duration::from_secs(2), closed_rx)
        .await
        .expect("pool idle close timeout")
        .expect("pool idle close observation");
}

fn credential_isolation(
    id: CredentialId,
    traffic_class: TransportTrafficClass,
) -> TransportIsolationKey {
    TransportIsolationKey::routing_credential(
        RoutingCredentialId::provider_credential(id),
        1,
        1,
        traffic_class,
    )
}

fn request_with_authorization(
    uri: &str,
    authorization: &str,
    isolation: TransportIsolationKey,
) -> TransportRequest {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        authorization.parse().expect("authorization header"),
    );
    TransportRequest {
        method: Method::POST,
        uri: Uri::from_str(uri).expect("request URI"),
        headers,
        body: Bytes::from_static(b"{}"),
        isolation,
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

fn request_without_body(uri: &str, isolation: TransportIsolationKey) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        isolation,
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

struct Http2Probe {
    address: std::net::SocketAddr,
    accepted_connections: Arc<AtomicUsize>,
    authorizations: mpsc::UnboundedReceiver<String>,
    handshakes: mpsc::UnboundedReceiver<rustls::HandshakeKind>,
}

async fn spawn_http2_probe(server_config: Arc<rustls::ServerConfig>) -> Http2Probe {
    let mut server_config = Arc::try_unwrap(server_config)
        .unwrap_or_else(|_| panic!("test TLS server config must be uniquely owned"));
    server_config.alpn_protocols = vec![b"h2".to_vec()];
    let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP/2 listener");
    let address = listener.local_addr().expect("HTTP/2 address");
    let accepted_connections = Arc::new(AtomicUsize::new(0));
    let accepted_connections_for_task = Arc::clone(&accepted_connections);
    let (observation_tx, observation_rx) = mpsc::unbounded_channel();
    let (handshake_tx, handshake_rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.expect("HTTP/2 TCP connection");
            accepted_connections_for_task.fetch_add(1, Ordering::SeqCst);
            let tls_acceptor = tls_acceptor.clone();
            let observation_tx = observation_tx.clone();
            let handshake_tx = handshake_tx.clone();
            tokio::spawn(async move {
                let stream = tls_acceptor
                    .accept(stream)
                    .await
                    .expect("HTTP/2 TLS handshake");
                handshake_tx
                    .send(
                        stream
                            .get_ref()
                            .1
                            .handshake_kind()
                            .expect("completed TLS handshake kind"),
                    )
                    .expect("TLS handshake observation receiver");
                let mut connection = h2::server::handshake(stream)
                    .await
                    .expect("HTTP/2 handshake");
                while let Some(request) = connection.accept().await {
                    let (request, mut respond) = request.expect("HTTP/2 request");
                    let authorization = request
                        .headers()
                        .get(AUTHORIZATION)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_owned();
                    observation_tx
                        .send(authorization)
                        .expect("HTTP/2 observation receiver");
                    let response = Response::builder()
                        .status(StatusCode::OK)
                        .body(())
                        .expect("HTTP/2 response");
                    let mut body = respond
                        .send_response(response, false)
                        .expect("HTTP/2 response headers");
                    body.send_data(Bytes::from_static(b"ok"), true)
                        .expect("HTTP/2 response body");
                }
            });
        }
    });

    Http2Probe {
        address,
        accepted_connections,
        authorizations: observation_rx,
        handshakes: handshake_rx,
    }
}

async fn receive_observation<T>(receiver: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(2), receiver.recv())
        .await
        .expect("HTTP/2 observation timeout")
        .expect("HTTP/2 observation channel")
}
