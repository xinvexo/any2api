use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::Bytes;
use http::{Response, StatusCode};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    sync::oneshot,
};
use tokio_rustls::TlsAcceptor;

use super::recording::WireCapture;

pub(super) struct ScenarioCapture {
    pub(super) connections: Vec<WireCapture>,
    pub(super) events: Vec<String>,
}

pub(super) struct H2Probe {
    pub(super) address: SocketAddr,
    captures: Vec<WireCapture>,
    events: Arc<Mutex<Vec<String>>>,
    first_request: Option<oneshot::Receiver<()>>,
    done: oneshot::Receiver<()>,
}

impl H2Probe {
    pub(super) async fn wait_for_first_request(&mut self) {
        let receiver = self
            .first_request
            .take()
            .expect("probe has no first-request gate");
        tokio::time::timeout(Duration::from_secs(2), receiver)
            .await
            .expect("HTTP/2 first request timeout")
            .expect("HTTP/2 first request observation");
    }

    pub(super) async fn wait_for_first_goaway(&self) {
        tokio::time::timeout(
            Duration::from_secs(2),
            self.captures[0].wait_for_server_frame(super::frames::GOAWAY),
        )
        .await
        .expect("HTTP/2 GOAWAY timeout");
    }

    pub(super) async fn finish(self) -> ScenarioCapture {
        let Self {
            captures,
            events,
            done,
            ..
        } = self;
        tokio::time::timeout(Duration::from_secs(2), done)
            .await
            .expect("HTTP/2 probe completion timeout")
            .expect("HTTP/2 probe completion");
        let events = events.lock().expect("HTTP/2 event lock").clone();
        ScenarioCapture {
            connections: captures,
            events,
        }
    }
}

pub(super) async fn spawn_sequential(server_config: Arc<rustls::ServerConfig>) -> H2Probe {
    let (listener, acceptor, address) = listener(server_config).await;
    let captures = vec![WireCapture::default()];
    let task_capture = captures[0].clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let task_events = Arc::clone(&events);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        let mut connection = accept_connection(&listener, &acceptor, task_capture).await;
        for _ in 0..2 {
            let response = accept_request(&mut connection, 1, &task_events).await;
            respond_ok(response, 1, &task_events);
        }
        drive_after_responses(&mut connection).await;
        done_tx.send(()).expect("sequential probe receiver");
    });
    H2Probe {
        address,
        captures,
        events,
        first_request: None,
        done: done_rx,
    }
}

pub(super) async fn spawn_concurrent(server_config: Arc<rustls::ServerConfig>) -> H2Probe {
    let (listener, acceptor, address) = listener(server_config).await;
    let captures = vec![WireCapture::default()];
    let task_capture = captures[0].clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let task_events = Arc::clone(&events);
    let (first_tx, first_rx) = oneshot::channel();
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        let mut connection = accept_connection(&listener, &acceptor, task_capture).await;
        let first = accept_request(&mut connection, 1, &task_events).await;
        first_tx
            .send(())
            .expect("concurrent first-request receiver");
        let second = accept_request(&mut connection, 1, &task_events).await;
        respond_ok(first, 1, &task_events);
        respond_ok(second, 1, &task_events);
        drive_after_responses(&mut connection).await;
        done_tx.send(()).expect("concurrent probe receiver");
    });
    H2Probe {
        address,
        captures,
        events,
        first_request: Some(first_rx),
        done: done_rx,
    }
}

pub(super) async fn spawn_goaway(server_config: Arc<rustls::ServerConfig>) -> H2Probe {
    let (listener, acceptor, address) = listener(server_config).await;
    let captures = vec![WireCapture::default(), WireCapture::default()];
    let first_capture = captures[0].clone();
    let second_capture = captures[1].clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let task_events = Arc::clone(&events);
    let (done_tx, done_rx) = oneshot::channel();
    tokio::spawn(async move {
        let mut first = accept_connection(&listener, &acceptor, first_capture).await;
        let response = accept_request(&mut first, 1, &task_events).await;
        respond_ok(response, 1, &task_events);
        first.graceful_shutdown();
        record(&task_events, "connection=1 server.graceful_goaway");
        // The peer may close after the first GOAWAY reaches the wire, before
        // h2 finishes flushing the final graceful-shutdown frames.
        match first.accept().await {
            None => {}
            Some(Ok(_)) => panic!("request arrived after observed GOAWAY"),
            Some(Err(error)) if error.is_io() => {}
            Some(Err(error)) => panic!("GOAWAY connection ended with an error: {error:?}"),
        }

        let mut second = accept_connection(&listener, &acceptor, second_capture).await;
        let response = accept_request(&mut second, 2, &task_events).await;
        respond_ok(response, 2, &task_events);
        drive_after_responses(&mut second).await;
        done_tx.send(()).expect("GOAWAY probe receiver");
    });
    H2Probe {
        address,
        captures,
        events,
        first_request: None,
        done: done_rx,
    }
}

async fn listener(
    server_config: Arc<rustls::ServerConfig>,
) -> (TcpListener, TlsAcceptor, SocketAddr) {
    let mut server_config = Arc::try_unwrap(server_config)
        .unwrap_or_else(|_| panic!("fixture TLS config must be uniquely owned"));
    server_config.alpn_protocols = vec![b"h2".to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(server_config));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP/2 fixture listener");
    let address = listener.local_addr().expect("HTTP/2 fixture address");
    (listener, acceptor, address)
}

async fn accept_connection(
    listener: &TcpListener,
    acceptor: &TlsAcceptor,
    capture: WireCapture,
) -> h2::server::Connection<
    super::recording::RecordingIo<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>,
    Bytes,
> {
    let (stream, _) = listener.accept().await.expect("HTTP/2 fixture connection");
    let stream = acceptor
        .accept(stream)
        .await
        .expect("HTTP/2 fixture TLS handshake");
    h2::server::handshake(capture.recording(stream))
        .await
        .expect("HTTP/2 fixture handshake")
}

async fn accept_request<T>(
    connection: &mut h2::server::Connection<T, Bytes>,
    connection_id: usize,
    events: &Arc<Mutex<Vec<String>>>,
) -> h2::server::SendResponse<Bytes>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let (request, respond) = connection
        .accept()
        .await
        .expect("HTTP/2 fixture request")
        .expect("valid HTTP/2 fixture request");
    assert_eq!(request.uri().path(), "/v1/responses");
    let stream_id = respond.stream_id().as_u32();
    record(
        events,
        &format!("connection={connection_id} server.accept stream={stream_id}"),
    );
    respond
}

fn respond_ok(
    mut respond: h2::server::SendResponse<Bytes>,
    connection_id: usize,
    events: &Arc<Mutex<Vec<String>>>,
) {
    let stream_id = respond.stream_id().as_u32();
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(())
        .expect("HTTP/2 response");
    let mut body = respond
        .send_response(response, false)
        .expect("HTTP/2 response headers");
    body.send_data(Bytes::from_static(b"ok"), true)
        .expect("HTTP/2 response body");
    record(
        events,
        &format!("connection={connection_id} server.respond stream={stream_id}"),
    );
}

async fn drive_after_responses<T>(connection: &mut h2::server::Connection<T, Bytes>)
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    match tokio::time::timeout(Duration::from_millis(200), connection.accept()).await {
        Err(_) | Ok(None) => {}
        Ok(Some(_)) => panic!("unexpected extra HTTP/2 request"),
    }
}

fn record(events: &Arc<Mutex<Vec<String>>>, event: &str) {
    events
        .lock()
        .expect("HTTP/2 event lock")
        .push(event.to_owned());
}
