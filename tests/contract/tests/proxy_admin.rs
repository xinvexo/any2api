use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{
    ConfigPublisher, ProxyTestService, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::ReqwestTransportManager;
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
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tower::ServiceExt;

const DIRECT_ID: &str = "00000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn proxy_admin_is_loopback_only_until_admin_authentication_exists() {
    let (_directory, app, _storage) = test_app().await;

    let (status, body) = request_json(
        app,
        Method::GET,
        "/api/admin/proxies",
        None,
        SocketAddr::from(([203, 0, 113, 10], 41000)),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");
}

#[tokio::test]
async fn proxy_crud_publishes_the_same_revision_to_storage_and_runtime() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, initial) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/proxies",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["config_revision"], 1);
    assert_eq!(initial["global_proxy_id"], DIRECT_ID);
    assert_eq!(initial["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(initial["items"][0]["built_in"], true);

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "Hong Kong",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["config_revision"], 2);
    let proxy_id = created["items"]
        .as_array()
        .expect("proxy items")
        .iter()
        .find(|item| item["built_in"] == false)
        .and_then(|item| item["id"].as_str())
        .expect("custom proxy id")
        .to_owned();

    let (status, global) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/set-global"),
        Some(json!({ "expected_revision": 2 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(global["config_revision"], 3);
    assert_eq!(global["global_proxy_id"], proxy_id);

    let (status, disabled_global) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/proxies/{proxy_id}"),
        Some(json!({
            "expected_revision": 3,
            "name": "Hong Kong",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": false
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(disabled_global["error"]["code"], "proxy_in_use");

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/proxies/{proxy_id}"),
        Some(json!({
            "expected_revision": 3,
            "name": "Hong Kong Primary",
            "kind": "socks5",
            "host": "socks.example.com",
            "port": 1080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 4);

    let (status, protected) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}?expected_revision=4"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(protected["error"]["code"], "proxy_in_use");

    let (status, direct_global) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{DIRECT_ID}/set-global"),
        Some(json!({ "expected_revision": 4 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(direct_global["config_revision"], 5);

    let (status, deleted) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}?expected_revision=5"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["config_revision"], 6);
    assert_eq!(deleted["items"].as_array().map(Vec::len), Some(1));

    let stored = storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 6);
    assert_eq!(stored.proxies().profiles().len(), 1);

    let (status, health) = request_json(app, Method::GET, "/api/health", None, loopback).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["config_revision"], 6);
    assert_eq!(health["scheduler_epoch"], 5);
}

#[tokio::test]
async fn stale_revision_and_direct_mutation_return_structured_conflicts() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, _) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "First",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, stale) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "Stale",
            "kind": "socks5",
            "host": "stale.example.com",
            "port": 1080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["error"]["code"], "revision_conflict");

    let (status, direct) = request_json(
        app,
        Method::PATCH,
        &format!("/api/admin/proxies/{DIRECT_ID}"),
        Some(json!({
            "expected_revision": 2,
            "name": "DIRECT",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(direct["error"]["code"], "proxy_protected");

    let stored = storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 2);
    assert_eq!(stored.proxies().profiles().len(), 2);
}

#[tokio::test]
async fn proxy_authentication_is_redacted_and_used_by_the_admin_probe() {
    let (_directory, app, _storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (proxy_address, proxy_request, rejected_proxy_request) = spawn_proxy_response().await;

    let (status, endpoint_configuration) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Probe Target",
            "provider_kind": "codex",
            "base_url": "http://upstream.invalid/v1",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let endpoint_id = endpoint_configuration["items"][0]["id"]
        .as_str()
        .expect("endpoint id")
        .to_owned();

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 2,
            "name": "Authenticated Proxy",
            "kind": "http",
            "host": proxy_address.ip().to_string(),
            "port": proxy_address.port(),
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let proxy_id = custom_proxy_id(&created);

    let (status, authenticated) = request_json(
        app.clone(),
        Method::PUT,
        &format!("/api/admin/proxies/{proxy_id}/authentication"),
        Some(json!({
            "expected_revision": 3,
            "username": "proxy-user",
            "password": "proxy-password"
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let profile = custom_proxy(&authenticated);
    assert_eq!(profile["username"], "proxy-user");
    assert_eq!(profile["password_configured"], true);
    assert_eq!(profile["authentication_version"], 1);
    assert!(!authenticated.to_string().contains("proxy-password"));

    let (status, arbitrary_target) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        Some(json!({
            "provider_endpoint_id": endpoint_id,
            "url": "http://127.0.0.1:1/private"
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(arbitrary_target["error"]["code"], "invalid_request");

    let (status, missing_target) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        Some(json!({
            "provider_endpoint_id": "11111111-1111-1111-1111-111111111111"
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        missing_target["error"]["code"],
        "provider_endpoint_not_found"
    );

    let (status, tested) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        Some(json!({ "provider_endpoint_id": endpoint_id })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tested["reachable"], true);
    assert_eq!(tested["status_code"], 404);
    assert_eq!(tested["proxy_id"], proxy_id);
    assert_eq!(tested["config_revision"], 4);
    assert_eq!(tested["proxy_config_version"], 2);
    assert_eq!(tested["provider_endpoint_config_version"], 1);
    let request = proxy_request.await.expect("captured proxy request");
    assert!(request.starts_with("GET http://upstream.invalid/v1 HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("proxy-authorization:")
    );
    assert!(
        !request
            .to_ascii_lowercase()
            .contains("authorization: bearer")
    );

    let (status, rejected) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        Some(json!({ "provider_endpoint_id": endpoint_id })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rejected["reachable"], false);
    assert_eq!(rejected["status_code"], Value::Null);
    assert_eq!(rejected["error_stage"], "proxy_handshake");
    assert_eq!(rejected["failure_scope"], "proxy");
    assert_eq!(rejected["config_revision"], 4);
    let _ = rejected_proxy_request
        .await
        .expect("captured rejected probe");

    let (status, cleared) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}/authentication?expected_revision=4"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let profile = custom_proxy(&cleared);
    assert_eq!(profile["username"], Value::Null);
    assert_eq!(profile["password_configured"], false);
    assert_eq!(profile["authentication_version"], 2);

    let (status, repeated_clear) = request_json(
        app,
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}/authentication?expected_revision=5"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_clear["config_revision"], 5);
    let profile = custom_proxy(&repeated_clear);
    assert_eq!(profile["authentication_version"], 2);
}

async fn test_app() -> (tempfile::TempDir, Router, Arc<SqliteStore>) {
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
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let proxy_tests = Arc::new(ProxyTestService::new(Arc::new(
        ReqwestTransportManager::default(),
    )));
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, public_requests).with_proxy_tests(proxy_tests),
        web_root,
    );

    (directory, app, storage)
}

fn custom_proxy_id(configuration: &Value) -> String {
    custom_proxy(configuration)["id"]
        .as_str()
        .expect("custom proxy id")
        .to_owned()
}

fn custom_proxy(configuration: &Value) -> &Value {
    configuration["items"]
        .as_array()
        .expect("proxy items")
        .iter()
        .find(|item| item["built_in"] == false)
        .expect("custom proxy")
}

async fn spawn_proxy_response() -> (
    SocketAddr,
    oneshot::Receiver<String>,
    oneshot::Receiver<String>,
) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("proxy listener");
    let address = listener.local_addr().expect("proxy address");
    let (request_tx, request_rx) = oneshot::channel();
    let (rejected_tx, rejected_rx) = oneshot::channel();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("proxy connection");
        request_tx.send(read_proxy_request(&mut stream).await).ok();
        stream
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .expect("proxy response write");
        let (mut stream, _) = listener.accept().await.expect("rejected proxy connection");
        rejected_tx.send(read_proxy_request(&mut stream).await).ok();
        stream
            .write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .expect("proxy rejection write");
    });
    (address, request_rx, rejected_rx)
}

async fn read_proxy_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.expect("proxy request read");
        assert!(read > 0, "proxy request ended before headers");
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8(bytes).expect("proxy request UTF-8")
}

async fn request_json(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(remote));
    let body = if let Some(value) = body {
        builder = builder.header(CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&value).expect("request json"))
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
    let value = serde_json::from_slice(&bytes).expect("response json");

    (status, value)
}
