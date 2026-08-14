use std::net::SocketAddr;

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
    net::TcpListener,
    sync::mpsc,
    time::{Duration, timeout},
};
use tower::ServiceExt;

#[tokio::test]
async fn alpha_search_forwards_to_the_codex_upstream_and_preserves_the_body() {
    let (upstream_address, mut upstream) = recording_upstream(
        StatusCode::OK,
        r#"{"output":"cited answer","results":[{"type":"text_result","ref_id":"turn0search0","url":"https://example.com"}],"encrypted_output":null}"#,
    )
    .await;
    let (_directory, app, token, _revision) = configured_app(&[upstream_address]).await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/alpha/search",
        Some(json!({
            "id": "0199aaaa-search-session",
            "model": "gpt-search-model",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "find this"}]
            }],
            "commands": {"search_query": [{"q": "any2api release", "recency": 7}]},
            "settings": {"allowed_callers": ["direct"], "external_web_access": true},
            "max_output_tokens": 2500,
            "future_field": {"keep": true}
        })),
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["output"], "cited answer");
    assert_eq!(response.body["results"][0]["ref_id"], "turn0search0");
    assert_eq!(response.body["encrypted_output"], Value::Null);

    let request = upstream.recv().await.expect("upstream request");
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path, "/v1/alpha/search");
    assert_eq!(request.headers["authorization"], "Bearer sk-alpha-search-0");
    assert_eq!(request.headers["content-type"], "application/json");
    assert_eq!(request.headers["accept"], "application/json");
    assert_eq!(request.body["id"], "0199aaaa-search-session");
    assert_eq!(request.body["model"], "gpt-search-model");
    assert_eq!(request.body["input"][0]["content"][0]["text"], "find this");
    assert_eq!(
        request.body["commands"]["search_query"][0]["q"],
        "any2api release"
    );
    assert_eq!(request.body["settings"]["external_web_access"], true);
    assert_eq!(request.body["max_output_tokens"], 2500);
    assert_eq!(request.body["future_field"]["keep"], true);
    assert!(
        !request
            .body
            .as_object()
            .expect("body object")
            .contains_key("stream")
    );
}

#[tokio::test]
async fn alpha_search_upstream_errors_pass_through_verbatim() {
    let (upstream_address, mut upstream) = recording_upstream(
        StatusCode::NOT_FOUND,
        r#"{"error":{"message":"search is not available for this account"}}"#,
    )
    .await;
    let (_directory, app, token, _revision) = configured_app(&[upstream_address]).await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/alpha/search",
        Some(json!({
            "id": "0199bbbb-search-session",
            "model": "gpt-search-model",
            "commands": {"search_query": [{"q": "missing"}]}
        })),
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(
        response.body,
        json!({"error": {"message": "search is not available for this account"}})
    );
    upstream.recv().await.expect("upstream request");
}

#[tokio::test]
async fn alpha_search_rejects_streaming_and_wrong_methods_locally() {
    let (upstream_address, mut upstream) =
        recording_upstream(StatusCode::OK, r#"{"output":"x"}"#).await;
    let (_directory, app, token, _revision) = configured_app(&[upstream_address]).await;

    let streaming = request_json(
        app.clone(),
        Method::POST,
        "/v1/alpha/search",
        Some(json!({
            "id": "0199cccc-search-session",
            "model": "gpt-search-model",
            "stream": true
        })),
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(streaming.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        streaming.body["error"]["message"],
        "request body is not valid for this endpoint"
    );

    let wrong_method = request_json(
        app,
        Method::GET,
        "/v1/alpha/search",
        None,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(wrong_method.body["error"]["message"].is_string());

    assert!(
        timeout(Duration::from_millis(100), upstream.recv())
            .await
            .is_err(),
        "locally rejected requests must not reach the upstream listener"
    );
}

#[tokio::test]
async fn alpha_search_follows_the_session_binding_of_the_model_request() {
    let responses_body =
        r#"{"id":"resp_bind_1","object":"response","model":"gpt-search-model","output":[]}"#;
    let search_body = r#"{"output":"bound answer","results":[]}"#;
    let (address_a, mut upstream_a) =
        recording_upstream_series(StatusCode::OK, &[responses_body, search_body, search_body])
            .await;
    let (address_b, mut upstream_b) =
        recording_upstream_series(StatusCode::OK, &[responses_body, search_body, search_body])
            .await;
    let (_directory, app, token, revision) = configured_app(&[address_a, address_b]).await;

    let session = "0199dddd-search-session";
    let toggle = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/affinity.enabled",
        Some(json!({"expected_revision": revision, "value": true})),
        &[],
    )
    .await;
    assert_eq!(toggle.status, StatusCode::OK);
    let seed = request_json(
        app.clone(),
        Method::POST,
        "/v1/responses",
        Some(json!({"model": "gpt-search-model", "input": "hello"})),
        &[
            ("authorization", format!("Bearer {token}")),
            ("session_id", session.to_owned()),
        ],
    )
    .await;
    assert_eq!(seed.status, StatusCode::OK);

    let seeded_on_a = timeout(Duration::from_millis(200), upstream_a.recv())
        .await
        .is_ok();
    let bound = if seeded_on_a {
        &mut upstream_a
    } else {
        upstream_b
            .recv()
            .await
            .expect("model request reaches one upstream");
        &mut upstream_b
    };

    for _ in 0..2 {
        let search = request_json(
            app.clone(),
            Method::POST,
            "/v1/alpha/search",
            Some(json!({
                "id": session,
                "model": "gpt-search-model",
                "commands": {"search_query": [{"q": "stay bound"}]}
            })),
            &[("authorization", format!("Bearer {token}"))],
        )
        .await;
        assert_eq!(search.status, StatusCode::OK);
        assert_eq!(search.body["output"], "bound answer");
        let request = timeout(Duration::from_millis(500), bound.recv())
            .await
            .expect("search must reach the session-bound upstream")
            .expect("bound upstream request");
        assert_eq!(request.path, "/v1/alpha/search");
        assert_eq!(request.body["id"], session);
    }
}

async fn configured_app(
    upstream_addresses: &[SocketAddr],
) -> (tempfile::TempDir, Router, String, u64) {
    let (directory, app, _storage) = TestApplication::new().await.into_router();
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
    let token = gateway.body["items"][0]["token"]
        .as_str()
        .expect("gateway token in collection item")
        .to_owned();
    let mut revision = 2;
    for (index, upstream_address) in upstream_addresses.iter().enumerate() {
        let endpoint = request_json_with_remote(
            app.clone(),
            Method::POST,
            "/api/admin/provider-endpoints",
            Some(json!({
                "expected_revision": revision,
                "name": format!("Codex search {index}"),
                "provider_kind": "codex",
                "base_url": format!("http://{upstream_address}/v1"),
                "protocol_dialect": "openai_responses",
                "enabled": true
            })),
            remote,
            &[],
        )
        .await;
        let endpoint_id = endpoint.body["items"]
            .as_array()
            .expect("endpoint items")
            .iter()
            .find(|item| item["name"] == format!("Codex search {index}").as_str())
            .expect("created endpoint")["id"]
            .as_str()
            .expect("endpoint id")
            .to_owned();
        revision += 1;
        let credential = request_json_with_remote(
            app.clone(),
            Method::POST,
            &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
            Some(json!({
                "expected_revision": revision,
                "label": "primary",
                "credential_kind": "api_key",
                "api_key": format!("sk-alpha-search-{index}"),
                "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
                "requests_per_minute": null,
                "enabled": true
            })),
            remote,
            &[],
        )
        .await;
        let credential_id = credential.body["items"][0]["id"]
            .as_str()
            .expect("credential id")
            .to_owned();
        revision += 1;
        let models = request_json_with_remote(
            app.clone(),
            Method::PUT,
            &format!("/api/admin/provider-credentials/{credential_id}/models"),
            Some(json!({
                "expected_revision": revision,
                "expected_config_version": 1,
                "models": ["gpt-search-model"]
            })),
            remote,
            &[],
        )
        .await;
        assert!(models.body["items"].is_array(), "models update {index}");
        revision += 1;
    }
    (directory, app, token, revision)
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

async fn recording_upstream(
    status: StatusCode,
    response_body: &str,
) -> (SocketAddr, mpsc::UnboundedReceiver<UpstreamRequest>) {
    recording_upstream_series(status, &[response_body]).await
}

/// Serves the given response bodies to sequential connections on one
/// listener, recording every request it receives.
async fn recording_upstream_series(
    status: StatusCode,
    response_bodies: &[&str],
) -> (SocketAddr, mpsc::UnboundedReceiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = mpsc::unbounded_channel();
    let response_bodies: Vec<String> = response_bodies
        .iter()
        .map(|body| (*body).to_owned())
        .collect();
    tokio::spawn(async move {
        for response_body in response_bodies {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let request = read_upstream_request(&mut stream).await;
            if sender.send(request).is_err() {
                return;
            }
            let reason = status.canonical_reason().unwrap_or("Unknown");
            let response = format!(
                "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status.as_u16(),
                response_body.len(),
                response_body
            );
            let _ = stream.write_all(response.as_bytes()).await;
        }
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
