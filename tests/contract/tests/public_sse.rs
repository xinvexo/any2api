use std::{collections::HashMap, fs, net::SocketAddr, sync::Arc, time::Duration};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
    response::Response,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
    time::timeout,
};
use tower::ServiceExt;

#[tokio::test]
async fn codex_and_claude_streams_forward_incrementally_and_restore_public_models() {
    let (codex_address, codex_request, release_codex) = paused_sse_server(
        "/v1/responses",
        &[
            b"event: response.cre",
            b"ated\r\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-upstream\"}}\r\n",
            b"\r\n",
        ],
        &[b"data: [DONE]\n\n"],
    )
    .await;
    let (claude_address, claude_request, release_claude) = paused_sse_server(
        "/v1/messages",
        &[
            b"event: message_start\n",
            b"data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-upstream\",\"content\":[]}}\n\n",
        ],
        &[b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"],
    )
    .await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let codex_endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex SSE",
        "codex",
        &format!("http://{codex_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &codex_endpoint,
        "codex-stream",
        "sk-codex-stream",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &codex_endpoint,
        "codex-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;
    revision += 1;
    let claude_endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Claude SSE",
        "claude",
        &format!("http://{claude_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &claude_endpoint,
        "claude-stream",
        "sk-claude-stream",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &claude_endpoint,
        "claude-public",
        "claude-upstream",
        "anthropic_messages",
    )
    .await;

    let codex = request(
        app.clone(),
        "/v1/responses",
        json!({"model":"codex-public","stream":true,"input":"hello"}),
        remote,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_stream_headers(&codex);
    let (codex_first, codex_rest) = read_paused_stream(codex, release_codex).await;
    assert!(codex_first.contains(r#""model":"codex-public""#));
    assert!(!codex_first.contains("gpt-upstream"));
    assert!(codex_rest.contains("[DONE]"));
    let codex_request = codex_request.await.expect("Codex upstream request");
    assert_eq!(codex_request.headers["accept"], "text/event-stream");
    assert_eq!(
        codex_request.headers["authorization"],
        "Bearer sk-codex-stream"
    );
    assert_eq!(codex_request.body["model"], "gpt-upstream");
    assert_eq!(codex_request.body["stream"], true);

    let claude = request(
        app,
        "/v1/messages",
        json!({
            "model":"claude-public",
            "stream":true,
            "max_tokens":32,
            "messages":[{"role":"user","content":"hello"}]
        }),
        remote,
        &[("x-api-key", token)],
    )
    .await;
    assert_stream_headers(&claude);
    let (claude_first, claude_rest) = read_paused_stream(claude, release_claude).await;
    assert!(claude_first.contains(r#""model":"claude-public""#));
    assert!(!claude_first.contains("claude-upstream"));
    assert!(claude_rest.contains("message_stop"));
    let claude_request = claude_request.await.expect("Claude upstream request");
    assert_eq!(claude_request.headers["accept"], "text/event-stream");
    assert_eq!(claude_request.headers["x-api-key"], "sk-claude-stream");
    assert_eq!(claude_request.headers["anthropic-version"], "2023-06-01");
    assert_eq!(claude_request.body["model"], "claude-upstream");
    assert_eq!(claude_request.body["stream"], true);
}

fn assert_stream_headers(response: &Response) {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    assert_eq!(response.headers()["cache-control"], "no-cache");
    assert!(response.headers().get("x-api-key").is_none());
    assert!(response.headers().get("set-cookie").is_none());
    assert_eq!(
        response.headers()["x-request-id"],
        "upstream-stream-request"
    );
}

async fn read_paused_stream(response: Response, release: oneshot::Sender<()>) -> (String, String) {
    let mut body = response.into_body();
    let first = timeout(Duration::from_secs(2), body.frame())
        .await
        .expect("first downstream frame timeout")
        .expect("first downstream frame")
        .expect("first downstream body result")
        .into_data()
        .expect("first downstream data");
    release.send(()).expect("release upstream stream");
    let rest = timeout(Duration::from_secs(2), body.collect())
        .await
        .expect("remaining downstream body timeout")
        .expect("remaining downstream body")
        .to_bytes();
    (
        String::from_utf8(first.to_vec()).expect("first UTF-8"),
        String::from_utf8(rest.to_vec()).expect("rest UTF-8"),
    )
}

async fn test_app() -> (tempfile::TempDir, Router, u64) {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let publisher = Arc::new(ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    ));
    let service = build_public_request_components()
        .expect("public request components")
        .service();
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let revision = snapshots.load().revision().get();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher).with_public_requests(service),
        web_root,
    );
    (directory, app, revision)
}

async fn create_gateway_key(app: &Router, remote: SocketAddr, revision: u64) -> String {
    let response = request_admin(
        app.clone(),
        "/api/admin/gateway-api-keys",
        json!({"expected_revision":revision,"name":"stream-client","enabled":true}),
        remote,
    )
    .await;
    response["token"]
        .as_str()
        .expect("gateway token")
        .to_owned()
}

async fn create_endpoint(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    name: &str,
    provider_kind: &str,
    base_url: &str,
) -> String {
    let dialect = if provider_kind == "codex" {
        "openai_responses"
    } else {
        "anthropic_messages"
    };
    let response = request_admin(
        app.clone(),
        "/api/admin/provider-endpoints",
        json!({
            "expected_revision":revision,
            "name":name,
            "provider_kind":provider_kind,
            "base_url":base_url,
            "protocol_dialect":dialect,
            "allow_insecure_http":true,
            "allow_private_network":true,
            "enabled":true
        }),
        remote,
    )
    .await;
    response["items"]
        .as_array()
        .expect("endpoint items")
        .iter()
        .find(|item| item["name"] == name)
        .and_then(|item| item["id"].as_str())
        .expect("created endpoint")
        .to_owned()
}

async fn create_credential(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    label: &str,
    api_key: &str,
) {
    request_admin(
        app.clone(),
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        json!({
            "expected_revision":revision,
            "label":label,
            "credential_kind":"api_key",
            "api_key":api_key,
            "proxy_profile_id":"00000000-0000-0000-0000-000000000000",
            "max_concurrency":1,
            "enabled":true
        }),
        remote,
    )
    .await;
}

async fn create_route(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    public_model: &str,
    upstream_model: &str,
    dialect: &str,
) {
    request_admin(
        app.clone(),
        "/api/admin/model-routes",
        json!({
            "expected_revision":revision,
            "public_model":public_model,
            "ingress_protocol":dialect,
            "fallback_on_saturation":null,
            "enabled":true,
            "targets":[{
                "provider_endpoint_id":endpoint_id,
                "upstream_model":upstream_model,
                "fallback_tier":0,
                "enabled":true
            }]
        }),
        remote,
    )
    .await;
}

async fn request_admin(app: Router, uri: &str, body: Value, remote: SocketAddr) -> Value {
    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(uri)
                .header(CONTENT_TYPE, "application/json")
                .extension(ConnectInfo(remote))
                .body(Body::from(
                    serde_json::to_vec(&body).expect("admin request JSON"),
                ))
                .expect("admin request"),
        )
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

async fn request(
    app: Router,
    uri: &str,
    body: Value,
    remote: SocketAddr,
    headers: &[(&str, String)],
) -> Response {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .extension(ConnectInfo(remote));
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    timeout(
        Duration::from_secs(2),
        app.oneshot(
            builder
                .body(Body::from(
                    serde_json::to_vec(&body).expect("public request JSON"),
                ))
                .expect("public request"),
        ),
    )
    .await
    .expect("public response timeout")
    .expect("public response")
}

#[derive(Debug)]
struct UpstreamRequest {
    headers: HashMap<String, String>,
    body: Value,
}

async fn paused_sse_server(
    expected_path: &'static str,
    first_chunks: &'static [&'static [u8]],
    remaining_chunks: &'static [&'static [u8]],
) -> (
    SocketAddr,
    oneshot::Receiver<UpstreamRequest>,
    oneshot::Sender<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (request_sender, request_receiver) = oneshot::channel();
    let (release_sender, release_receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("upstream accept");
        let request = read_upstream_request(&mut stream).await;
        assert_eq!(request.0, expected_path);
        request_sender
            .send(request.1)
            .expect("send upstream request");
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nX-Api-Key: upstream-secret\r\nSet-Cookie: upstream-secret=1\r\nX-Request-Id: upstream-stream-request\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("upstream response headers");
        for chunk in first_chunks {
            write_chunk(&mut stream, chunk).await;
        }
        stream.flush().await.expect("flush first event");
        let _ = release_receiver.await;
        for chunk in remaining_chunks {
            write_chunk(&mut stream, chunk).await;
        }
        stream
            .write_all(b"0\r\n\r\n")
            .await
            .expect("finish chunked response");
    });
    (address, request_receiver, release_sender)
}

async fn write_chunk(stream: &mut tokio::net::TcpStream, chunk: &[u8]) {
    stream
        .write_all(format!("{:X}\r\n", chunk.len()).as_bytes())
        .await
        .expect("chunk length");
    stream.write_all(chunk).await.expect("chunk body");
    stream.write_all(b"\r\n").await.expect("chunk ending");
}

async fn read_upstream_request(stream: &mut tokio::net::TcpStream) -> (String, UpstreamRequest) {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).await.expect("upstream read");
        assert!(count > 0, "upstream request ended before headers");
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("request boundary");
    let head = String::from_utf8(bytes[..header_end].to_vec()).expect("upstream headers");
    let content_length = head
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
        })
        .unwrap_or(0);
    let body_start = header_end + 4;
    while bytes.len() < body_start + content_length {
        let count = stream.read(&mut buffer).await.expect("upstream body read");
        assert!(count > 0, "upstream request body ended early");
        bytes.extend_from_slice(&buffer[..count]);
    }
    let mut lines = head.lines();
    let request_line = lines.next().expect("request line");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let body = serde_json::from_slice(&bytes[body_start..body_start + content_length])
        .expect("upstream JSON body");
    (path, UpstreamRequest { headers, body })
}
