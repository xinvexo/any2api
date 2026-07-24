use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::oneshot,
};
use tower::ServiceExt;

#[tokio::test]
async fn count_tokens_uses_auxiliary_path_and_preserves_request_fields() {
    let (upstream_address, upstream) = upstream_server(
        StatusCode::OK,
        r#"{"input_tokens":37}"#,
        "/v1/messages/count_tokens",
    )
    .await;
    let (_directory, app, token) = configured_app(upstream_address).await;
    let response = request_json(
        app,
        Method::POST,
        "/v1/messages/count_tokens",
        Some(json!({
            "model": "claude-upstream",
            "messages": [{"role":"user","content":"hello"}],
            "system": "system prompt",
            "tools": [{"name":"lookup"}],
            "future_field": {"keep": true}
        })),
        &[
            ("x-api-key", token),
            ("anthropic-beta", "token-counting-2024-11-01".to_owned()),
        ],
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body, json!({"input_tokens": 37}));
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path, "/v1/messages/count_tokens");
    assert_eq!(request.headers["x-api-key"], "sk-count-tokens-provider");
    assert_eq!(request.headers["anthropic-version"], "2023-06-01");
    assert_eq!(
        request.headers["anthropic-beta"],
        "token-counting-2024-11-01"
    );
    assert!(!request.headers.contains_key("authorization"));
    assert_eq!(request.body["model"], "claude-upstream");
    assert_eq!(request.body["system"], "system prompt");
    assert_eq!(request.body["tools"][0]["name"], "lookup");
    assert_eq!(request.body["future_field"]["keep"], true);
}

#[tokio::test]
async fn count_tokens_upstream_not_found_returns_anthropic_404() {
    let (upstream_address, upstream) = upstream_server(
        StatusCode::NOT_FOUND,
        r#"{"secret":"upstream-body-must-not-leak"}"#,
        "/v1/messages/count_tokens",
    )
    .await;
    let (_directory, app, token) = configured_app(upstream_address).await;
    let response = request_json(
        app,
        Method::POST,
        "/v1/messages/count_tokens",
        Some(json!({
            "model": "claude-upstream",
            "messages": [{"role":"user","content":"hello"}]
        })),
        &[("x-api-key", token)],
    )
    .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.body["type"], "error");
    assert_eq!(response.body["error"]["type"], "not_found_error");
    assert_eq!(
        response.body["error"]["message"],
        "upstream operation is unavailable"
    );
    assert!(!response.body.to_string().contains("must-not-leak"));
    upstream.await.expect("upstream request");
}

async fn configured_app(upstream_address: SocketAddr) -> (tempfile::TempDir, Router, String) {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
        any2api_contract_tests::build_provider_registry().as_ref(),
    )));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            any2api_contract_tests::build_configuration_capabilities(),
        )
        .expect("configuration publisher"),
    );
    let service = build_public_request_components()
        .expect("public request components")
        .service();
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, service),
        web_root,
    );
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));

    let gateway = request_json_with_remote(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({"expected_revision":1,"name":"client","enabled":true})),
        remote,
        &[],
    )
    .await;
    let token = gateway.body["token"].as_str().expect("token").to_owned();
    let endpoint = request_json_with_remote(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 2,
            "name": "Claude local",
            "provider_kind": "claude",
            "base_url": format!("http://{upstream_address}/v1"),
            "protocol_dialect": "anthropic_messages",
            "enabled": true
        })),
        remote,
        &[],
    )
    .await;
    let endpoint_id = endpoint.body["items"][0]["id"]
        .as_str()
        .expect("endpoint id");
    let credential = request_json_with_remote(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": 3,
            "label": "primary",
            "credential_kind": "api_key",
            "api_key": "sk-count-tokens-provider",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 1,
            "enabled": true
        })),
        remote,
        &[],
    )
    .await;
    let credential_id = credential.body["items"][0]["id"]
        .as_str()
        .expect("credential id");
    request_json_with_remote(
        app.clone(),
        Method::PUT,
        &format!("/api/admin/provider-credentials/{credential_id}/models"),
        Some(json!({
            "expected_revision": 4,
            "expected_config_version": 1,
            "models": ["claude-upstream"]
        })),
        remote,
        &[],
    )
    .await;
    (directory, app, token)
}

async fn request_json(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    headers: &[(&str, String)],
) -> JsonResponse {
    request_json_with_remote(
        app,
        method,
        uri,
        body,
        SocketAddr::from(([127, 0, 0, 1], 41000)),
        headers,
    )
    .await
}

async fn request_json_with_remote(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
    headers: &[(&str, String)],
) -> JsonResponse {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(remote));
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    let body = if let Some(value) = body {
        builder = builder.header(CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&value).expect("request JSON"))
    } else {
        Body::empty()
    };
    let response = app
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    JsonResponse {
        status,
        body: serde_json::from_slice(&bytes).expect("JSON response"),
    }
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
}

#[derive(Debug)]
struct UpstreamRequest {
    method: Method,
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: Value,
}

async fn upstream_server(
    status: StatusCode,
    response_body: &str,
    expected_path: &str,
) -> (SocketAddr, oneshot::Receiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = oneshot::channel();
    let response_body = response_body.to_owned();
    let expected_path = expected_path.to_owned();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("upstream accept");
        let request = read_upstream_request(&mut stream).await;
        assert_eq!(request.path, expected_path);
        sender.send(request).expect("send upstream request");
        let reason = status.canonical_reason().unwrap_or("Unknown");
        let response = format!(
            "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status.as_u16(),
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("upstream write");
    });
    (address, receiver)
}

async fn read_upstream_request(stream: &mut tokio::net::TcpStream) -> UpstreamRequest {
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
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .expect("method")
        .parse::<Method>()
        .expect("valid method");
    let path = parts.next().expect("path").to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let body = serde_json::from_slice(&bytes[body_start..body_start + content_length])
        .expect("upstream JSON body");
    UpstreamRequest {
        method,
        path,
        headers,
        body,
    }
}
