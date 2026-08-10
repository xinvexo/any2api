use std::{
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::{ProxyKind, ProxyProfile, ProxyProfileId};
use bytes::Bytes;
use http::{HeaderMap, Method, Response, StatusCode, Uri, header::AUTHORIZATION};
use tokio::{net::TcpListener, sync::mpsc};
use tokio_rustls::TlsAcceptor;

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, TransportManager, TransportManagerConfig, TransportProxy,
        TransportRequest,
    },
    connection::tests::TestTlsIdentity,
};

use super::tests::collect_body;

#[tokio::test]
async fn distinct_authorization_contexts_share_one_http2_connection() {
    let identity = TestTlsIdentity::generate();
    let mut probe = spawn_http2_probe(identity.server_config).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy = ProxyProfile::direct();
    let uri = format!("https://localhost:{}/v1/responses", probe.address.port());

    let first_client = manager.client_for(&proxy).expect("first cached client");
    let first = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-a"),
        )
        .await
        .expect("first HTTP/2 response");
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(collect_body(first).await, Bytes::from_static(b"ok"));

    let second_client = manager.client_for(&proxy).expect("second cached client");
    let second = manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_with_authorization(&uri, "Bearer credential-b"),
        )
        .await
        .expect("second HTTP/2 response");
    assert_eq!(second.status, StatusCode::OK);
    assert_eq!(collect_body(second).await, Bytes::from_static(b"ok"));

    assert!(Arc::ptr_eq(&first_client, &second_client));
    let first_authorization = receive_observation(&mut probe.authorizations).await;
    let second_authorization = receive_observation(&mut probe.authorizations).await;
    assert_eq!(first_authorization, "Bearer credential-a");
    assert_eq!(second_authorization, "Bearer credential-b");
    assert_eq!(probe.accepted_connections.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn tls_resumption_state_is_shared_across_distinct_cached_clients() {
    let identity = TestTlsIdentity::generate();
    let mut probe = spawn_http2_probe(identity.server_config).await;
    let manager = ReqwestTransportManager::new_with_test_root_certificate(
        TransportManagerConfig::default(),
        identity.client_certificate,
    )
    .expect("transport manager");
    let proxy_v1 = ProxyProfile::direct();
    let proxy_v2 = ProxyProfile::restore(
        ProxyProfileId::DIRECT,
        "DIRECT",
        ProxyKind::Direct,
        None,
        true,
        None,
        0,
        2,
    )
    .expect("second DIRECT configuration version");
    let uri = format!("https://localhost:{}/v1/responses", probe.address.port());

    let first_client = manager.client_for(&proxy_v1).expect("first cached client");
    let first = manager
        .execute(
            TransportProxy::new(&proxy_v1, None),
            request_with_authorization(&uri, "Bearer credential-a"),
        )
        .await
        .expect("first HTTP/2 response");
    assert_eq!(collect_body(first).await, Bytes::from_static(b"ok"));
    assert_eq!(
        receive_observation(&mut probe.handshakes).await,
        rustls::HandshakeKind::Full
    );

    let second_client = manager.client_for(&proxy_v2).expect("second cached client");
    assert!(!Arc::ptr_eq(&first_client, &second_client));
    let second = manager
        .execute(
            TransportProxy::new(&proxy_v2, None),
            request_with_authorization(&uri, "Bearer credential-b"),
        )
        .await
        .expect("second HTTP/2 response");
    assert_eq!(collect_body(second).await, Bytes::from_static(b"ok"));

    assert_eq!(probe.accepted_connections.load(Ordering::SeqCst), 2);
    assert_eq!(
        receive_observation(&mut probe.handshakes).await,
        rustls::HandshakeKind::Resumed
    );
}

fn request_with_authorization(uri: &str, authorization: &str) -> TransportRequest {
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
