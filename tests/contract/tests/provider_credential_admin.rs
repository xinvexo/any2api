use std::net::SocketAddr;

use any2api_contract_tests::TestApplication;
use any2api_storage::api::ConfigurationRepository;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn provider_credential_crud_rotates_without_exposing_the_api_key() {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let (_directory, app, _) = fixture.into_router();
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let endpoint = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Codex Primary",
            "provider_kind": "codex",
            "base_url": "https://api.example.com/v1",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(endpoint.status, StatusCode::OK);
    let endpoint_id = endpoint.body["items"][0]["id"]
        .as_str()
        .expect("endpoint id");

    let created = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": 2,
            "label": "Primary Key",
            "credential_kind": "api_key",
            "api_key": "sk-contract-create-secret",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "requests_per_minute": 4,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(created.status, StatusCode::OK);
    assert_eq!(created.cache_control.as_deref(), Some("no-store"));
    assert!(!created.raw_body.contains("sk-contract-create-secret"));
    assert!(
        created.body["items"][0]["fingerprint"]
            .as_str()
            .is_some_and(|value| value.starts_with("v2:"))
    );
    assert!(created.body["items"][0].get("usage").is_none());
    let credential_id = created.body["items"][0]["id"]
        .as_str()
        .expect("credential id");

    let rotated = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-credentials/{credential_id}/rotate-secret"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 1,
            "expected_secret_version": 1,
            "api_key": "sk-contract-rotated-secret"
        })),
        loopback,
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK);
    assert!(!rotated.raw_body.contains("sk-contract-rotated-secret"));
    assert_eq!(rotated.body["items"][0]["secret_version"], 2);
    assert_eq!(rotated.body["items"][0]["credential_generation"], 2);
    assert!(rotated.body["items"][0].get("usage").is_none());

    let deleted = request_json(
        app,
        Method::DELETE,
        &format!(
            "/api/admin/provider-credentials/{credential_id}?expected_revision=4&expected_config_version=2"
        ),
        None,
        loopback,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert!(deleted.body["items"].as_array().is_some_and(Vec::is_empty));
    assert!(
        storage
            .load_configuration()
            .await
            .expect("configuration")
            .provider_credentials()
            .credentials()
            .is_empty()
    );
}

struct JsonResponse {
    status: StatusCode,
    body: Value,
    raw_body: String,
    cache_control: Option<String>,
}

async fn request_json(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
) -> JsonResponse {
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
    let cache_control = response
        .headers()
        .get("cache-control")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    let raw_body = String::from_utf8(bytes.to_vec()).expect("UTF-8 response");
    let body = serde_json::from_str(&raw_body).expect("response json");
    JsonResponse {
        status,
        body,
        raw_body,
        cache_control,
    }
}
