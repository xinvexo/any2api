use std::{collections::HashMap, net::SocketAddr, time::Duration};

use any2api_contract_tests::TestApplication;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    time::timeout,
};
use tower::ServiceExt;

#[tokio::test]
async fn websocket_requests_replay_the_pipeline_and_extend_the_baseline_incrementally() {
    let (upstream_address, mut upstream_requests) = scripted_upstream(vec![
        UpstreamScript::Sse(&[
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_ws_1\",\"model\":\"gpt-upstream\"}}\n\n",
            b"event: response.output_item.done\ndata: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"msg_out_1\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"Hello\"}]}}\n\n",
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_ws_1\",\"model\":\"gpt-upstream\"}}\n\n",
        ]),
        UpstreamScript::Sse(&[
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_ws_2\",\"model\":\"gpt-upstream\"}}\n\n",
            b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_ws_2\",\"model\":\"gpt-upstream\"}}\n\n",
        ]),
    ])
    .await;
    let (_directory, app, token) = websocket_app(&upstream_address).await;
    let address = serve_app(app).await;

    let mut client =
        WsTestClient::connect(address, &[("authorization", &format!("Bearer {token}"))])
            .await
            .expect("websocket upgrade");
    client
        .send_text(
            &json!({
                "type": "response.create",
                "model": "gpt-upstream",
                "stream": true,
                "store": false,
                "input": [{"type":"message","role":"user","content":[{"type":"input_text","text":"first"}]}],
                "client_metadata": {
                    "session_id": "sess-ws",
                    "x-codex-ws-stream-request-start-ms": "1755200000000"
                }
            })
            .to_string(),
        )
        .await;
    assert_eq!(client.recv_json().await["type"], "response.created");
    let item_done = client.recv_json().await;
    assert_eq!(item_done["type"], "response.output_item.done");
    let completed = client.recv_json().await;
    assert_eq!(completed["type"], "response.completed");
    assert_eq!(completed["response"]["id"], "resp_ws_1");

    let first_upstream = upstream_requests.recv().await.expect("first upstream");
    assert_eq!(first_upstream.path, "/v1/responses");
    assert_eq!(
        first_upstream.headers["authorization"],
        "Bearer sk-ws-stream"
    );
    assert_eq!(first_upstream.headers["session-id"], "sess-ws");
    assert_eq!(first_upstream.headers["accept"], "text/event-stream");
    assert!(first_upstream.body.get("type").is_none());
    assert!(first_upstream.body.get("generate").is_none());
    assert!(first_upstream.body.get("previous_response_id").is_none());
    assert_eq!(
        first_upstream.body["client_metadata"],
        json!({"session_id": "sess-ws"})
    );
    assert_eq!(
        first_upstream.body["input"],
        json!([{"type":"message","role":"user","content":[{"type":"input_text","text":"first"}]}])
    );

    client
        .send_text(
            &json!({
                "type": "response.create",
                "model": "gpt-upstream",
                "stream": true,
                "store": false,
                "previous_response_id": "resp_ws_1",
                "input": [{"type":"message","role":"user","content":[{"type":"input_text","text":"second"}]}],
                "client_metadata": {"session_id": "sess-ws"}
            })
            .to_string(),
        )
        .await;
    assert_eq!(client.recv_json().await["type"], "response.created");
    let completed = client.recv_json().await;
    assert_eq!(completed["type"], "response.completed");
    assert_eq!(completed["response"]["id"], "resp_ws_2");

    let second_upstream = upstream_requests.recv().await.expect("second upstream");
    assert_eq!(
        second_upstream.headers["authorization"],
        "Bearer sk-ws-stream"
    );
    assert!(second_upstream.body.get("previous_response_id").is_none());
    assert_eq!(
        second_upstream.body["input"],
        json!([
            {"type":"message","role":"user","content":[{"type":"input_text","text":"first"}]},
            {"type":"message","id":"msg_out_1","role":"assistant","content":[{"type":"output_text","text":"Hello"}]},
            {"type":"message","role":"user","content":[{"type":"input_text","text":"second"}]},
        ])
    );
}

#[tokio::test]
async fn warmup_is_answered_locally_and_unknown_continuations_recover() {
    let (upstream_address, mut upstream_requests) = scripted_upstream(vec![UpstreamScript::Sse(&[
        b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_real\",\"model\":\"gpt-upstream\"}}\n\n",
        b"event: response.completed\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_real\",\"model\":\"gpt-upstream\"}}\n\n",
    ])])
    .await;
    let (_directory, app, token) = websocket_app(&upstream_address).await;
    let address = serve_app(app).await;

    let mut client =
        WsTestClient::connect(address, &[("authorization", &format!("Bearer {token}"))])
            .await
            .expect("websocket upgrade");
    client
        .send_text(
            &json!({
                "type": "response.create",
                "model": "gpt-upstream",
                "stream": true,
                "generate": false,
                "input": [{"type":"message","role":"user","content":[{"type":"input_text","text":"prefix"}]}]
            })
            .to_string(),
        )
        .await;
    let created = client.recv_json().await;
    assert_eq!(created["type"], "response.created");
    let warmup_id = created["response"]["id"]
        .as_str()
        .expect("warmup response id")
        .to_owned();
    assert!(warmup_id.starts_with("resp_any2api_"));
    let completed = client.recv_json().await;
    assert_eq!(completed["type"], "response.completed");
    assert_eq!(completed["response"]["id"], warmup_id.as_str());
    assert!(
        timeout(Duration::from_millis(100), upstream_requests.recv())
            .await
            .is_err(),
        "warmup must not contact upstream"
    );

    client
        .send_text(
            &json!({
                "type": "response.create",
                "model": "gpt-upstream",
                "stream": true,
                "previous_response_id": warmup_id,
                "input": [{"type":"message","role":"user","content":[{"type":"input_text","text":"turn"}]}]
            })
            .to_string(),
        )
        .await;
    assert_eq!(client.recv_json().await["type"], "response.created");
    assert_eq!(client.recv_json().await["type"], "response.completed");

    let upstream = upstream_requests.recv().await.expect("turn upstream");
    assert!(upstream.body.get("previous_response_id").is_none());
    assert_eq!(
        upstream.body["input"],
        json!([
            {"type":"message","role":"user","content":[{"type":"input_text","text":"prefix"}]},
            {"type":"message","role":"user","content":[{"type":"input_text","text":"turn"}]},
        ])
    );

    client
        .send_text(
            &json!({
                "type": "response.create",
                "model": "gpt-upstream",
                "stream": true,
                "previous_response_id": "resp_unknown",
                "input": []
            })
            .to_string(),
        )
        .await;
    let recovery = client.recv_json().await;
    assert_eq!(recovery["type"], "error");
    assert_eq!(recovery["error"]["code"], "previous_response_not_found");
}

#[tokio::test]
async fn upstream_errors_are_wrapped_and_invalid_upgrades_keep_http_answers() {
    let (upstream_address, mut upstream_requests) = scripted_upstream(vec![UpstreamScript::Error {
        status_line: "400 Bad Request",
        headers: &[("x-codex-primary-used-percent", "42.0")],
        body: r#"{"error":{"type":"invalid_request_error","code":"model_cap","message":"blocked"}}"#,
    }])
    .await;
    let (_directory, app, token) = websocket_app(&upstream_address).await;
    let address = serve_app(app.clone()).await;

    let mut client =
        WsTestClient::connect(address, &[("authorization", &format!("Bearer {token}"))])
            .await
            .expect("websocket upgrade");
    client
        .send_text(
            &json!({
                "type": "response.create",
                "model": "gpt-upstream",
                "stream": true,
                "input": [{"type":"message","role":"user","content":[{"type":"input_text","text":"boom"}]}]
            })
            .to_string(),
        )
        .await;
    let wrapped = client.recv_json().await;
    assert_eq!(wrapped["type"], "error");
    assert_eq!(wrapped["status"], 400);
    assert_eq!(wrapped["error"]["code"], "model_cap");
    assert_eq!(wrapped["error"]["message"], "blocked");
    assert_eq!(wrapped["headers"]["x-codex-primary-used-percent"], "42.0");
    upstream_requests.recv().await.expect("upstream error hit");

    client
        .send_text(&json!({"type": "response.create", "model": "gpt-upstream", "stream": false, "input": []}).to_string())
        .await;
    let invalid = client.recv_json().await;
    assert_eq!(invalid["type"], "error");
    assert_eq!(invalid["status"], 400);

    let handshake =
        WsTestClient::connect(address, &[("authorization", "Bearer wrong-token")]).await;
    assert_eq!(handshake.err(), Some(401));

    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let plain_get = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/v1/responses")
                .header("authorization", format!("Bearer {token}"))
                .extension(ConnectInfo(remote))
                .body(Body::empty())
                .expect("plain GET request"),
        )
        .await
        .expect("plain GET response");
    assert_eq!(plain_get.status(), StatusCode::METHOD_NOT_ALLOWED);
    let body: Value = serde_json::from_slice(
        &plain_get
            .into_body()
            .collect()
            .await
            .expect("plain GET body")
            .to_bytes(),
    )
    .expect("plain GET JSON");
    assert_eq!(body["error"]["code"], "method_not_allowed");
}

// ---------------------------------------------------------------------------
// Application setup
// ---------------------------------------------------------------------------

async fn websocket_app(upstream_address: &SocketAddr) -> (tempfile::TempDir, Router, String) {
    let fixture = TestApplication::new().await;
    let mut revision = fixture.snapshots().load().revision().get();
    let (directory, app, _storage) = fixture.into_router();
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(&app, remote, revision, &endpoint).await;
    revision += 1;
    select_models(&app, remote, revision, &endpoint, "gpt-upstream").await;
    (directory, app, token)
}

async fn serve_app(app: Router) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("public listener");
    let address = listener.local_addr().expect("public address");
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .expect("serve public app");
    });
    address
}

async fn create_gateway_key(app: &Router, remote: SocketAddr, revision: u64) -> String {
    let response = request_admin(
        app,
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({"expected_revision":revision,"name":"ws-client","enabled":true})),
        remote,
    )
    .await;
    response["items"][0]["token"]
        .as_str()
        .expect("gateway token")
        .to_owned()
}

async fn create_endpoint(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    base_url: &str,
) -> String {
    let response = request_admin(
        app,
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision":revision,
            "name":"Codex WebSocket",
            "provider_kind":"codex",
            "base_url":base_url,
            "protocol_dialect":"openai_responses",
            "upstream_protocol_dialect":null,
            "enabled":true
        })),
        remote,
    )
    .await;
    response["items"]
        .as_array()
        .expect("endpoint items")
        .iter()
        .find(|item| item["name"] == "Codex WebSocket")
        .and_then(|item| item["id"].as_str())
        .expect("created endpoint")
        .to_owned()
}

async fn create_credential(app: &Router, remote: SocketAddr, revision: u64, endpoint_id: &str) {
    request_admin(
        app,
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision":revision,
            "label":"ws-stream",
            "credential_kind":"api_key",
            "api_key":"sk-ws-stream",
            "proxy_profile_id":"00000000-0000-0000-0000-000000000000",
            "requests_per_minute":null,
            "enabled":true
        })),
        remote,
    )
    .await;
}

async fn select_models(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    model: &str,
) {
    let listed = request_admin(
        app,
        Method::GET,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        None,
        remote,
    )
    .await;
    let credentials = listed["items"]
        .as_array()
        .expect("credential items")
        .iter()
        .map(|credential| {
            (
                credential["id"].as_str().expect("credential id").to_owned(),
                credential["config_version"]
                    .as_u64()
                    .expect("credential config version"),
            )
        })
        .collect::<Vec<_>>();
    assert!(!credentials.is_empty());
    for (offset, (credential_id, config_version)) in credentials.into_iter().enumerate() {
        request_admin(
            app,
            Method::PUT,
            &format!("/api/admin/provider-credentials/{credential_id}/models"),
            Some(json!({
                "expected_revision": revision + offset as u64,
                "expected_config_version": config_version,
                "models": [{"upstream_model": model}]
            })),
            remote,
        )
        .await;
    }
}

async fn request_admin(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
) -> Value {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(remote));
    let body = if let Some(body) = body {
        builder = builder.header(CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&body).expect("admin request JSON"))
    } else {
        Body::empty()
    };
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("admin request"))
        .await
        .expect("admin response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("admin response body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("admin response JSON")
}

// ---------------------------------------------------------------------------
// Scripted upstream fixture
// ---------------------------------------------------------------------------

enum UpstreamScript {
    Sse(&'static [&'static [u8]]),
    Error {
        status_line: &'static str,
        headers: &'static [(&'static str, &'static str)],
        body: &'static str,
    },
}

struct UpstreamRequest {
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

async fn scripted_upstream(
    scripts: Vec<UpstreamScript>,
) -> (SocketAddr, mpsc::UnboundedReceiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        for script in scripts {
            let (mut stream, _) = listener.accept().await.expect("upstream accept");
            let request = read_upstream_request(&mut stream).await;
            let _ = sender.send(request);
            match script {
                UpstreamScript::Sse(frames) => {
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n",
                        )
                        .await
                        .expect("upstream SSE head");
                    for frame in frames {
                        let chunk = format!("{:x}\r\n", frame.len());
                        stream
                            .write_all(chunk.as_bytes())
                            .await
                            .expect("chunk size");
                        stream.write_all(frame).await.expect("chunk body");
                        stream.write_all(b"\r\n").await.expect("chunk end");
                    }
                    stream.write_all(b"0\r\n\r\n").await.expect("chunk finish");
                }
                UpstreamScript::Error {
                    status_line,
                    headers,
                    body,
                } => {
                    let mut head = format!(
                        "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
                        body.len()
                    );
                    for (name, value) in headers {
                        head.push_str(&format!("{name}: {value}\r\n"));
                    }
                    head.push_str("\r\n");
                    stream
                        .write_all(head.as_bytes())
                        .await
                        .expect("upstream error head");
                    stream
                        .write_all(body.as_bytes())
                        .await
                        .expect("upstream error body");
                }
            }
            stream.flush().await.expect("upstream flush");
        }
    });
    (address, receiver)
}

async fn read_upstream_request(stream: &mut TcpStream) -> UpstreamRequest {
    let mut buffer = Vec::new();
    let header_end = loop {
        if let Some(end) = find_double_crlf(&buffer) {
            break end;
        }
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await.expect("upstream read");
        assert!(read > 0, "upstream connection closed before headers");
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8(buffer[..header_end].to_vec()).expect("request head UTF-8");
    let mut lines = head.split("\r\n");
    let request_line = lines.next().expect("request line");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let content_length: usize = headers
        .get("content-length")
        .expect("content-length")
        .parse()
        .expect("content-length number");
    let mut body = buffer.split_off(header_end + 4);
    while body.len() < content_length {
        let mut chunk = [0_u8; 4096];
        let read = stream.read(&mut chunk).await.expect("upstream body read");
        assert!(read > 0, "upstream connection closed before body");
        body.extend_from_slice(&chunk[..read]);
    }
    let body = serde_json::from_slice(&body[..content_length]).expect("upstream body JSON");
    UpstreamRequest {
        path,
        headers,
        body,
    }
}

// ---------------------------------------------------------------------------
// Minimal WebSocket test client (RFC 6455 client frames over TcpStream)
// ---------------------------------------------------------------------------

struct WsTestClient {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl WsTestClient {
    async fn connect(address: SocketAddr, headers: &[(&str, &str)]) -> Result<Self, u16> {
        let mut stream = TcpStream::connect(address).await.expect("client connect");
        let mut request = format!(
            "GET /v1/responses HTTP/1.1\r\nhost: {address}\r\nconnection: Upgrade\r\nupgrade: websocket\r\nsec-websocket-version: 13\r\nsec-websocket-key: AAAAAAAAAAAAAAAAAAAAAA==\r\n"
        );
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("handshake write");
        let mut buffer = Vec::new();
        let header_end = loop {
            if let Some(end) = find_double_crlf(&buffer) {
                break end;
            }
            let mut chunk = [0_u8; 4096];
            let read = timeout(Duration::from_secs(5), stream.read(&mut chunk))
                .await
                .expect("handshake timeout")
                .expect("handshake read");
            assert!(read > 0, "connection closed during handshake");
            buffer.extend_from_slice(&chunk[..read]);
        };
        let head = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let mut header_names = head
            .lines()
            .filter_map(|line| line.split_once(':').map(|(name, _)| name));
        assert!(
            header_names
                .clone()
                .all(|name| !name.eq_ignore_ascii_case("x-any2api-request-id")),
            "websocket response must not expose the any2api-specific request ID header"
        );
        assert!(
            header_names.any(|name| name.eq_ignore_ascii_case("x-request-id")),
            "websocket response must expose the single normalized request ID header"
        );
        let status: u16 = head
            .split_whitespace()
            .nth(1)
            .expect("status code")
            .parse()
            .expect("status number");
        if status != 101 {
            return Err(status);
        }
        let remainder = buffer.split_off(header_end + 4);
        Ok(Self {
            stream,
            buffer: remainder,
        })
    }

    async fn send_text(&mut self, text: &str) {
        const MASK: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
        let payload = text.as_bytes();
        let mut frame = vec![0x81_u8];
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else {
            assert!(payload.len() < 65_536, "test frames stay under 64 KiB");
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        }
        frame.extend_from_slice(&MASK);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(index, byte)| byte ^ MASK[index % 4]),
        );
        self.stream.write_all(&frame).await.expect("frame write");
    }

    async fn recv_json(&mut self) -> Value {
        loop {
            let (opcode, payload) = timeout(Duration::from_secs(5), self.recv_frame())
                .await
                .expect("frame timeout");
            match opcode {
                0x1 => {
                    return serde_json::from_slice(&payload).expect("text frame JSON");
                }
                0x9 | 0xA => {}
                0x8 => panic!(
                    "connection closed by server: {}",
                    String::from_utf8_lossy(&payload)
                ),
                other => panic!("unexpected websocket opcode {other}"),
            }
        }
    }

    async fn recv_frame(&mut self) -> (u8, Vec<u8>) {
        let head = self.read_exact(2).await;
        assert_ne!(head[0] & 0x80, 0, "server messages must not be fragmented");
        assert_eq!(head[1] & 0x80, 0, "server frames are unmasked");
        let opcode = head[0] & 0x0F;
        let length = match head[1] & 0x7F {
            126 => {
                let extended = self.read_exact(2).await;
                u16::from_be_bytes([extended[0], extended[1]]) as usize
            }
            127 => {
                let extended = self.read_exact(8).await;
                let mut bytes = [0_u8; 8];
                bytes.copy_from_slice(&extended);
                usize::try_from(u64::from_be_bytes(bytes)).expect("frame length")
            }
            length => length as usize,
        };
        let payload = self.read_exact(length).await;
        (opcode, payload)
    }

    async fn read_exact(&mut self, length: usize) -> Vec<u8> {
        while self.buffer.len() < length {
            let mut chunk = [0_u8; 4096];
            let read = self
                .stream
                .read(&mut chunk)
                .await
                .expect("websocket stream read");
            assert!(read > 0, "server closed the websocket stream");
            self.buffer.extend_from_slice(&chunk[..read]);
        }
        let remainder = self.buffer.split_off(length);
        std::mem::replace(&mut self.buffer, remainder)
    }
}

fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}
