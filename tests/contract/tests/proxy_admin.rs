use std::{fs, net::SocketAddr, sync::Arc};

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
    let app = build_router(AppState::new(snapshots, runtime, publisher), web_root);

    (directory, app, storage)
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
