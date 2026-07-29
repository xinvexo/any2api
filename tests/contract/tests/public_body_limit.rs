use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RuntimeRegistry, STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES,
    SnapshotStore,
};
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

#[tokio::test]
async fn oversized_responses_request_returns_openai_shaped_413() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let response = send_raw(
        &app,
        "/v1/responses",
        &token,
        vec![b'x'; STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES + 1],
    )
    .await;

    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.body["error"]["type"], "invalid_request_error");
    assert_eq!(response.body["error"]["code"], "payload_too_large");
}

#[tokio::test]
async fn oversized_messages_request_returns_anthropic_shaped_413() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let response = send_raw(
        &app,
        "/v1/messages",
        &token,
        vec![b'x'; STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES + 1],
    )
    .await;

    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.body["type"], "error");
    assert_eq!(response.body["error"]["type"], "request_too_large");
}

#[tokio::test]
async fn multi_megabyte_request_passes_the_raised_body_limit() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    // 3 MiB exceeds axum's former 2 MB default; reaching model resolution
    // proves the body was buffered instead of rejected with 413.
    let padding = "p".repeat(3 * 1024 * 1024);
    let body = serde_json::to_vec(&json!({
        "model": "model-that-does-not-exist",
        "input": padding,
        "stream": false
    }))
    .expect("request JSON");
    let response = send_raw(&app, "/v1/responses", &token, body).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.body["error"]["code"], "model_not_found");
}

#[tokio::test]
async fn images_edit_request_can_exceed_the_standard_body_limit() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let padding = "p".repeat(STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES + 1);
    let body = serde_json::to_vec(&json!({
        "model": "image-model-that-does-not-exist",
        "prompt": "edit a test image",
        "images": [{"file_id": "file-test-image"}],
        "metadata": padding
    }))
    .expect("request JSON");
    let response = send_raw(&app, "/v1/images/edits", &token, body).await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.body["error"]["code"], "model_not_found");
}

#[tokio::test]
async fn oversized_images_generation_keeps_the_standard_body_limit() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let response = send_raw(
        &app,
        "/v1/images/generations",
        &token,
        vec![b'x'; STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES + 1],
    )
    .await;

    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.body["error"]["type"], "invalid_request_error");
    assert_eq!(response.body["error"]["code"], "payload_too_large");
}

async fn send_raw(app: &Router, uri: &str, token: &str, body: Vec<u8>) -> JsonResponse {
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .extension(ConnectInfo(remote))
        .header(CONTENT_TYPE, "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    JsonResponse {
        status,
        body: serde_json::from_slice(&bytes).expect("JSON error envelope"),
    }
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
}

async fn test_app_with_gateway_key() -> (tempfile::TempDir, Router, String) {
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
    let revision = snapshots.load().revision().get();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, service),
        web_root,
    );

    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/admin/gateway-api-keys")
        .extension(ConnectInfo(remote))
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "expected_revision": revision,
                "name": "body-limit-client",
                "enabled": true
            }))
            .expect("request JSON"),
        ))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    assert!(response.status().is_success());
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("gateway response body")
        .to_bytes();
    let body: Value = serde_json::from_slice(&bytes).expect("gateway response JSON");
    let token = body["items"][0]["token"]
        .as_str()
        .expect("gateway token in collection item")
        .to_owned();
    (directory, app, token)
}
