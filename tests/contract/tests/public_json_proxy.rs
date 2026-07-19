use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Method, Request, StatusCode, header::CONTENT_TYPE},
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
async fn codex_responses_uses_upstream_path_and_provider_key() {
    let (listener, upstream) = upstream_server_with_headers(
        "/v1/responses",
        r#"{"id":"resp_1","model":"gpt-upstream","output":[]}"#,
        &[
            ("Authorization", "Bearer provider-secret"),
            ("X-Api-Key", "provider-secret"),
            ("Set-Cookie", "provider-secret=1"),
            ("Connection", "x-private-hop"),
            ("X-Private-Hop", "provider-secret"),
            ("ETag", "\"upstream-body\""),
            ("X-Request-Id", "upstream-request-1"),
        ],
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Codex local",
        "codex",
        &format!("http://{listener}/v1"),
    )
    .await;
    let credential_id = create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-codex-contract",
    )
    .await;
    create_route(
        &app,
        loopback,
        revision + 3,
        &endpoint_id,
        "codex-local",
        "gpt-upstream",
        "openai_responses",
    )
    .await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/responses",
        Some(json!({
            "model": "codex-local",
            "input": "hello",
            "stream": false,
            "unknown_field": {"keep": true}
        })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["model"], "codex-local");
    assert_eq!(response.body["id"], "resp_1");
    assert!(response.headers.get("authorization").is_none());
    assert!(response.headers.get("x-api-key").is_none());
    assert!(response.headers.get("set-cookie").is_none());
    assert!(response.headers.get("x-private-hop").is_none());
    assert!(response.headers.get("etag").is_none());
    assert_eq!(response.headers["x-request-id"], "upstream-request-1");
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path, "/v1/responses");
    assert_eq!(
        request.headers.get("authorization"),
        Some(&"Bearer sk-codex-contract".to_owned())
    );
    assert!(!request.headers.contains_key("x-api-key"));
    assert_eq!(request.body["model"], "gpt-upstream");
    assert_eq!(request.body["unknown_field"]["keep"], true);
    let _ = credential_id;
}

#[tokio::test]
async fn responses_compact_uses_its_distinct_non_streaming_path() {
    let (listener, upstream) = upstream_server(
        "/v1/responses/compact",
        r#"{"id":"cmp_1","model":"gpt-upstream","output":[]}"#,
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Codex compact",
        "codex",
        &format!("http://{listener}/v1"),
    )
    .await;
    create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-compact-contract",
    )
    .await;
    create_route(
        &app,
        loopback,
        revision + 3,
        &endpoint_id,
        "compact-local",
        "gpt-upstream",
        "openai_responses",
    )
    .await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/responses/compact",
        Some(json!({
            "model": "compact-local",
            "input": [{"role":"user","content":"compact this"}]
        })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["model"], "compact-local");
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.path, "/v1/responses/compact");
    assert_eq!(request.body["model"], "gpt-upstream");
}

#[tokio::test]
async fn claude_messages_uses_anthropic_headers_and_path() {
    let (listener, upstream) = upstream_server(
        "/v1/messages",
        r#"{"id":"msg_1","type":"message","model":"claude-upstream","content":[]}"#,
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Claude local",
        "claude",
        &format!("http://{listener}/v1"),
    )
    .await;
    create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-claude-contract",
    )
    .await;
    create_route(
        &app,
        loopback,
        revision + 3,
        &endpoint_id,
        "claude-local",
        "claude-upstream",
        "anthropic_messages",
    )
    .await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/messages",
        Some(json!({
            "model": "claude-local",
            "max_tokens": 32,
            "messages": [{"role":"user","content":"hello"}]
        })),
        loopback,
        &[
            ("x-api-key", token),
            ("anthropic-beta", "messages-2024-09-04".to_owned()),
        ],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["model"], "claude-local");
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.path, "/v1/messages");
    assert_eq!(
        request.headers.get("x-api-key"),
        Some(&"sk-claude-contract".to_owned())
    );
    assert_eq!(
        request.headers.get("anthropic-version"),
        Some(&"2023-06-01".to_owned())
    );
    assert_eq!(
        request.headers.get("anthropic-beta"),
        Some(&"messages-2024-09-04".to_owned())
    );
    assert!(!request.headers.contains_key("authorization"));
    assert_eq!(request.body["model"], "claude-upstream");
}

#[tokio::test]
async fn public_fallback_and_method_errors_require_authentication_and_return_json() {
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let missing = request_json(
        app.clone(),
        Method::GET,
        "/v1/not-a-route",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(missing.body["error"]["code"], "unauthorized");

    let token = create_gateway_key(&app, loopback, revision).await;
    let unknown = request_json(
        app.clone(),
        Method::GET,
        "/v1/not-a-route",
        None,
        loopback,
        &[("x-api-key", token.clone())],
    )
    .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert_eq!(unknown.body["error"]["code"], "public_api_not_found");

    let wrong_method = request_json(
        app,
        Method::GET,
        "/v1/responses",
        None,
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(wrong_method.body["error"]["code"], "method_not_allowed");
}

async fn test_app() -> (tempfile::TempDir, Router, u64) {
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
    let response = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({"expected_revision": revision, "name":"client", "enabled":true})),
        remote,
        &[],
    )
    .await;
    response.body["token"].as_str().expect("token").to_owned()
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
    let response = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": revision,
            "name": name,
            "provider_kind": provider_kind,
            "base_url": base_url,
            "protocol_dialect": dialect,
            "allow_insecure_http": true,
            "allow_private_network": true,
            "enabled": true
        })),
        remote,
        &[],
    )
    .await;
    response.body["items"][0]["id"]
        .as_str()
        .expect("endpoint")
        .to_owned()
}

async fn create_credential(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    api_key: &str,
) -> String {
    let response = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": revision,
            "label": "primary",
            "credential_kind": "api_key",
            "api_key": api_key,
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 2,
            "enabled": true
        })),
        remote,
        &[],
    )
    .await;
    response.body["items"][0]["id"]
        .as_str()
        .expect("credential")
        .to_owned()
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
    let response = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/model-routes",
        Some(json!({
            "expected_revision": revision,
            "public_model": public_model,
            "ingress_protocol": dialect,
            "fallback_on_saturation": null,
            "enabled": true,
            "targets": [{
                "provider_endpoint_id": endpoint_id,
                "upstream_model": upstream_model,
                "fallback_tier": 0,
                "enabled": true
            }]
        })),
        remote,
        &[],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
}

struct JsonResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

async fn request_json(
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
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    JsonResponse {
        status,
        headers,
        body: serde_json::from_slice(&bytes).expect("JSON response"),
    }
}

#[derive(Debug)]
struct UpstreamRequest {
    method: Method,
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: Value,
}

async fn upstream_server(
    expected_path: &str,
    response_body: &str,
) -> (SocketAddr, oneshot::Receiver<UpstreamRequest>) {
    upstream_server_with_headers(expected_path, response_body, &[]).await
}

async fn upstream_server_with_headers(
    expected_path: &str,
    response_body: &str,
    response_headers: &[(&str, &str)],
) -> (SocketAddr, oneshot::Receiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = oneshot::channel();
    let expected_path = expected_path.to_owned();
    let response_body = response_body.to_owned();
    let response_headers = response_headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("upstream accept");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = stream.read(&mut buffer).await.expect("upstream read");
            if count == 0 {
                break;
            }
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
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        let body = String::from_utf8(bytes[body_start..].to_vec()).expect("upstream body text");
        let mut lines = head.lines();
        let request_line = lines.next().expect("request line");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .expect("request method")
            .parse::<Method>()
            .expect("valid request method");
        let path = request_parts.next().expect("request path");
        assert_eq!(path, expected_path);
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
            .collect::<std::collections::HashMap<_, _>>();
        let body = serde_json::from_str(body.trim()).expect("upstream JSON body");
        sender
            .send(UpstreamRequest {
                method,
                path: path.to_owned(),
                headers,
                body,
            })
            .expect("send upstream request");
        let extra_headers = response_headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
            response_body.len(),
            extra_headers,
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("upstream write");
    });
    (address, receiver)
}
