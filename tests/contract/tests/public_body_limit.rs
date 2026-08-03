use std::{convert::Infallible, future::Future, net::SocketAddr};

use any2api_contract_tests::TestApplication;
use any2api_runtime::api::{
    IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES, STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES,
};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use futures_util::{StreamExt, stream};
use http_body_util::BodyExt;
use serde_json::{Value, json};
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

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.body["error"]["code"], "model_not_found");
    assert_eq!(response.body["error"]["param"], "model");
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

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.body["error"]["code"], "model_not_found");
    assert_eq!(response.body["error"]["param"], "model");
}

#[tokio::test]
async fn images_edit_request_over_the_dedicated_limit_returns_413() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let response = send_raw(
        &app,
        "/v1/images/edits",
        &token,
        vec![b'x'; IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES + 1],
    )
    .await;

    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.body["error"]["code"], "payload_too_large");
}

#[tokio::test]
async fn pending_small_body_does_not_reserve_the_endpoint_maximum() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let (first, first_started) = pending_image_edit(app.clone(), token.clone(), 1);
    let first = tokio::spawn(first);
    first_started.await.expect("first body was polled");

    let concurrent = send_raw(
        &app,
        "/v1/images/edits",
        &token,
        br#"{"model":"missing-image-model"}"#.to_vec(),
    )
    .await;
    assert_eq!(concurrent.status, StatusCode::BAD_REQUEST);
    assert_eq!(concurrent.body["error"]["code"], "model_not_found");

    first.abort();
    let _ = first.await;
}

#[tokio::test]
async fn fully_grown_body_does_not_block_an_independent_request() {
    let (_directory, app, token) = test_app_with_gateway_key().await;
    let (first, first_started) = pending_image_edit(
        app.clone(),
        token.clone(),
        IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES,
    );
    let first = tokio::spawn(first);
    first_started.await.expect("full body was buffered");

    let concurrent = send_raw(
        &app,
        "/v1/images/edits",
        &token,
        br#"{"model":"missing-image-model"}"#.to_vec(),
    )
    .await;
    assert_eq!(concurrent.status, StatusCode::BAD_REQUEST);
    assert_eq!(concurrent.body["error"]["code"], "model_not_found");

    first.abort();
    let _ = first.await;
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

fn pending_image_edit(
    app: Router,
    token: String,
    body_bytes: usize,
) -> (
    impl Future<Output = Result<axum::response::Response, Infallible>> + Send + 'static,
    tokio::sync::oneshot::Receiver<()>,
) {
    let remote = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (started, receiver) = tokio::sync::oneshot::channel();
    let chunks = stream::unfold(body_bytes, |remaining| async move {
        if remaining == 0 {
            return None;
        }
        let chunk_len = remaining.min(1024 * 1024);
        Some((
            Ok::<Bytes, Infallible>(Bytes::from(vec![b'x'; chunk_len])),
            remaining - chunk_len,
        ))
    });
    let pending_tail = stream::once(async move {
        let _ = started.send(());
        std::future::pending::<Result<Bytes, Infallible>>().await
    });
    let body = chunks.chain(pending_tail);
    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/images/edits")
        .extension(ConnectInfo(remote))
        .header(CONTENT_TYPE, "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from_stream(body))
        .expect("pending request");
    (async move { app.oneshot(request).await }, receiver)
}

async fn test_app_with_gateway_key() -> (tempfile::TempDir, Router, String) {
    let fixture = TestApplication::new().await;
    let revision = fixture.snapshots().load().revision().get();
    let (directory, app, _storage) = fixture.into_router();

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
