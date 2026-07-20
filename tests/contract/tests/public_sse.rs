use std::{collections::HashMap, fs, net::SocketAddr, sync::Arc, time::Duration};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::RequestId;
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
    sync::{mpsc, oneshot},
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

#[tokio::test]
async fn stream_precommit_byte_budget_is_applied_from_published_settings() {
    let (upstream_address, _upstream_request, release) = paused_sse_server(
        "/v1/responses",
        &[b"data: {\"model\":\"gpt-upstream\",\"output\":[]}\n\n"],
        &[],
    )
    .await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex precommit budget",
        "codex",
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "precommit-budget",
        "sk-precommit-budget",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &endpoint,
        "precommit-budget-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;
    revision += 1;
    update_setting(
        &app,
        remote,
        revision,
        "stream.precommit.max_bytes",
        json!(16),
    )
    .await;

    let response = request(
        app,
        "/v1/responses",
        json!({"model":"precommit-budget-public","stream":true,"input":"hello"}),
        remote,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("budget error body")
        .to_bytes();
    let error: Value = serde_json::from_slice(&body).expect("budget error JSON");
    assert_eq!(error["error"]["code"], "upstream_error");
    release.send(()).expect("release upstream stream");
}

#[tokio::test]
async fn stream_precommit_duration_is_applied_from_published_settings() {
    let (upstream_address, _upstream_request, release) =
        paused_sse_server("/v1/responses", &[], &[]).await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex precommit duration",
        "codex",
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "precommit-duration",
        "sk-precommit-duration",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &endpoint,
        "precommit-duration-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;
    revision += 1;
    update_setting(
        &app,
        remote,
        revision,
        "stream.precommit.max_duration",
        json!(10),
    )
    .await;

    let response = request(
        app,
        "/v1/responses",
        json!({"model":"precommit-duration-public","stream":true,"input":"hello"}),
        remote,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("duration error body")
        .to_bytes();
    let error: Value = serde_json::from_slice(&body).expect("duration error JSON");
    assert_eq!(error["error"]["code"], "upstream_error");
    release.send(()).expect("release upstream stream");
}

#[tokio::test]
async fn in_flight_stream_keeps_the_precommit_budget_from_its_snapshot() {
    let (upstream_address, upstream_ready, release) = gated_first_event_server(
        "/v1/responses",
        b"{\"model\":\"gpt-upstream\",\"output\":[]}\n\n",
    )
    .await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex snapshot budget",
        "codex",
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "snapshot-budget",
        "sk-snapshot-budget",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &endpoint,
        "snapshot-budget-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;
    revision += 1;

    let request_app = app.clone();
    let pending = tokio::spawn(async move {
        request(
            request_app,
            "/v1/responses",
            json!({"model":"snapshot-budget-public","stream":true,"input":"hello"}),
            remote,
            &[("authorization", format!("Bearer {token}"))],
        )
        .await
    });
    upstream_ready.await.expect("upstream response headers");
    tokio::time::sleep(Duration::from_millis(20)).await;
    update_setting(
        &app,
        remote,
        revision,
        "stream.precommit.max_duration",
        json!(1),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    release.send(()).expect("release first event");

    let response = pending.await.expect("public request task");
    assert_stream_headers(&response);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("stream body")
        .to_bytes();
    assert!(String::from_utf8_lossy(&body).contains(r#""model":"snapshot-budget-public""#));
}

#[tokio::test]
async fn codex_response_created_event_binds_the_follow_up_to_the_same_credential() {
    let (upstream_address, upstream_requests) = sse_then_json_server().await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex SSE affinity",
        "codex",
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "first-stream",
        "sk-stream-first",
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "second-stream",
        "sk-stream-second",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &endpoint,
        "stream-affinity-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;

    let first = request(
        app.clone(),
        "/v1/responses",
        json!({"model":"stream-affinity-public","stream":true,"input":"start"}),
        remote,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = first
        .into_body()
        .collect()
        .await
        .expect("stream response")
        .to_bytes();
    assert!(String::from_utf8_lossy(&first_body).contains("resp_stream_affinity"));

    let second = request(
        app,
        "/v1/responses",
        json!({
            "model":"stream-affinity-public",
            "previous_response_id":"resp_stream_affinity",
            "input":"continue"
        }),
        remote,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(second.status(), StatusCode::OK);
    second.into_body().collect().await.expect("JSON response");

    let (first_request, second_request) = upstream_requests.await.expect("both upstream requests");
    assert_eq!(
        first_request.headers.get("authorization"),
        second_request.headers.get("authorization")
    );
}

#[tokio::test]
async fn prefer_soft_affinity_rebinds_after_its_wait_timeout() {
    let (upstream_address, mut upstream_requests, release_first) = held_stream_server().await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex prefer",
        "codex",
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "prefer-first",
        "sk-prefer-first",
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "prefer-second",
        "sk-prefer-second",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &endpoint,
        "prefer-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;
    revision += 1;
    update_setting(
        &app,
        remote,
        revision,
        "affinity.soft.prefer_wait_timeout",
        json!(20),
    )
    .await;

    let held = request(
        app.clone(),
        "/v1/responses",
        json!({"model":"prefer-public","stream":true,"input":"start"}),
        remote,
        &[
            ("authorization", format!("Bearer {token}")),
            ("x-any2api-session", "prefer-session".to_owned()),
        ],
    )
    .await;
    assert_eq!(held.status(), StatusCode::OK);
    let first = upstream_requests
        .recv()
        .await
        .expect("first upstream request");

    let rebound = request(
        app,
        "/v1/responses",
        json!({"model":"prefer-public","input":"continue"}),
        remote,
        &[
            ("authorization", format!("Bearer {token}")),
            ("x-any2api-session", "prefer-session".to_owned()),
        ],
    )
    .await;
    assert_eq!(rebound.status(), StatusCode::OK);
    rebound
        .into_body()
        .collect()
        .await
        .expect("rebound response");
    let second = upstream_requests
        .recv()
        .await
        .expect("rebound upstream request");
    assert_ne!(
        first.headers.get("authorization"),
        second.headers.get("authorization")
    );

    release_first.send(()).expect("release first stream");
    held.into_body()
        .collect()
        .await
        .expect("held stream completion");
}

#[tokio::test]
async fn strict_soft_affinity_never_switches_credentials() {
    let (upstream_address, mut upstream_requests, release_first) = held_stream_server().await;
    let (_directory, app, mut revision) = test_app().await;
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, remote, revision).await;
    revision += 1;
    let endpoint = create_endpoint(
        &app,
        remote,
        revision,
        "Codex strict",
        "codex",
        &format!("http://{upstream_address}/v1"),
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "strict-first",
        "sk-strict-first",
    )
    .await;
    revision += 1;
    create_credential(
        &app,
        remote,
        revision,
        &endpoint,
        "strict-second",
        "sk-strict-second",
    )
    .await;
    revision += 1;
    create_route(
        &app,
        remote,
        revision,
        &endpoint,
        "strict-public",
        "gpt-upstream",
        "openai_responses",
    )
    .await;
    revision += 1;
    update_setting(
        &app,
        remote,
        revision,
        "affinity.soft.mode",
        json!("strict"),
    )
    .await;
    revision += 1;
    update_setting(
        &app,
        remote,
        revision,
        "affinity.fixed_wait_timeout",
        json!(20),
    )
    .await;

    let held = request(
        app.clone(),
        "/v1/responses",
        json!({"model":"strict-public","stream":true,"input":"start"}),
        remote,
        &[
            ("authorization", format!("Bearer {token}")),
            ("x-any2api-session", "strict-session".to_owned()),
        ],
    )
    .await;
    assert_eq!(held.status(), StatusCode::OK);
    upstream_requests
        .recv()
        .await
        .expect("first upstream request");

    let blocked = request(
        app,
        "/v1/responses",
        json!({"model":"strict-public","input":"continue"}),
        remote,
        &[
            ("authorization", format!("Bearer {token}")),
            ("x-any2api-session", "strict-session".to_owned()),
        ],
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
    let blocked_body = blocked
        .into_body()
        .collect()
        .await
        .expect("strict error response")
        .to_bytes();
    let blocked_json: Value = serde_json::from_slice(&blocked_body).expect("strict error JSON");
    assert_eq!(blocked_json["error"]["code"], "local_concurrency_limit");
    assert!(
        timeout(Duration::from_millis(50), upstream_requests.recv())
            .await
            .is_err(),
        "strict affinity must not contact another credential"
    );

    release_first.send(()).expect("release first stream");
    held.into_body()
        .collect()
        .await
        .expect("held stream completion");
}

fn assert_stream_headers(response: &Response) {
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[CONTENT_TYPE], "text/event-stream");
    assert_eq!(response.headers()["cache-control"], "no-cache");
    assert!(response.headers().get("x-api-key").is_none());
    assert!(response.headers().get("set-cookie").is_none());
    let request_id = response.headers()["x-request-id"]
        .to_str()
        .expect("request ID text");
    assert_ne!(request_id, "upstream-stream-request");
    assert!(request_id.parse::<RequestId>().is_ok());
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

async fn update_setting(app: &Router, remote: SocketAddr, revision: u64, key: &str, value: Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PATCH)
                .uri(format!("/api/admin/settings/{key}"))
                .header(CONTENT_TYPE, "application/json")
                .extension(ConnectInfo(remote))
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "expected_revision": revision,
                        "value": value
                    }))
                    .expect("setting request JSON"),
                ))
                .expect("setting request"),
        )
        .await
        .expect("setting response");
    assert_eq!(response.status(), StatusCode::OK);
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

async fn gated_first_event_server(
    expected_path: &'static str,
    event_tail: &'static [u8],
) -> (SocketAddr, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (ready_sender, ready_receiver) = oneshot::channel();
    let (release_sender, release_receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("upstream accept");
        let request = read_upstream_request(&mut stream).await;
        assert_eq!(request.0, expected_path);
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("upstream response headers");
        write_chunk(&mut stream, b"data: ").await;
        stream.flush().await.expect("flush response headers");
        ready_sender.send(()).expect("signal response headers");
        let _ = release_receiver.await;
        write_chunk(&mut stream, event_tail).await;
        stream
            .write_all(b"0\r\n\r\n")
            .await
            .expect("finish chunked response");
    });
    (address, ready_receiver, release_sender)
}

async fn sse_then_json_server() -> (
    SocketAddr,
    oneshot::Receiver<(UpstreamRequest, UpstreamRequest)>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut first_stream, _) = listener.accept().await.expect("first upstream accept");
        let (first_path, first_request) = read_upstream_request(&mut first_stream).await;
        assert_eq!(first_path, "/v1/responses");
        let event = b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_stream_affinity\",\"model\":\"gpt-upstream\"}}\n\ndata: [DONE]\n\n";
        first_stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    event.len()
                )
                .as_bytes(),
            )
            .await
            .expect("first response headers");
        first_stream
            .write_all(event)
            .await
            .expect("first response body");

        let (mut second_stream, _) = listener.accept().await.expect("second upstream accept");
        let (second_path, second_request) = read_upstream_request(&mut second_stream).await;
        assert_eq!(second_path, "/v1/responses");
        let body = r#"{"id":"resp_stream_follow","model":"gpt-upstream","output":[]}"#;
        second_stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )
            .await
            .expect("second response");
        sender
            .send((first_request, second_request))
            .expect("send upstream requests");
    });
    (address, receiver)
}

async fn held_stream_server() -> (
    SocketAddr,
    mpsc::UnboundedReceiver<UpstreamRequest>,
    oneshot::Sender<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (request_sender, request_receiver) = mpsc::unbounded_channel();
    let (release_sender, release_receiver) = oneshot::channel();
    tokio::spawn(async move {
        let (mut first_stream, _) = listener.accept().await.expect("first upstream accept");
        let (first_path, first_request) = read_upstream_request(&mut first_stream).await;
        assert_eq!(first_path, "/v1/responses");
        request_sender
            .send(first_request)
            .expect("send first upstream request");
        first_stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("held response headers");
        write_chunk(
            &mut first_stream,
            b"event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_held\",\"model\":\"gpt-upstream\"}}\n\n",
        )
        .await;
        first_stream.flush().await.expect("flush held event");

        let request_sender_for_second = request_sender.clone();
        let second = tokio::spawn(async move {
            let (mut second_stream, _) = listener.accept().await.expect("second upstream accept");
            let (second_path, second_request) = read_upstream_request(&mut second_stream).await;
            assert_eq!(second_path, "/v1/responses");
            request_sender_for_second
                .send(second_request)
                .expect("send second upstream request");
            let body = r#"{"id":"resp_rebound","model":"gpt-upstream","output":[]}"#;
            second_stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await
                .expect("second response");
        });

        let _ = release_receiver.await;
        write_chunk(&mut first_stream, b"data: [DONE]\n\n").await;
        first_stream
            .write_all(b"0\r\n\r\n")
            .await
            .expect("finish held stream");
        second.abort();
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
