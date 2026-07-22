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
use tower::ServiceExt;

#[tokio::test]
async fn gateway_key_create_rotate_revoke_controls_public_access() {
    let (_directory, app) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let created = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({
            "expected_revision": 1,
            "name": "Desktop",
            "enabled": true
        })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(created.status, StatusCode::OK);
    assert_eq!(created.cache_control.as_deref(), Some("no-store"));
    let first_token = created.body["token"]
        .as_str()
        .expect("created token")
        .to_owned();
    assert!(first_token.starts_with("a2k_v1_"));
    assert_eq!(created.body["config_revision"], 2);
    let key_id = created.body["items"][0]["id"]
        .as_str()
        .expect("key id")
        .to_owned();

    let listed = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/gateway-api-keys",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert!(!listed.body.to_string().contains(&first_token));
    assert!(!listed.body.to_string().contains("\"token\""));

    let missing = request_json(app.clone(), Method::GET, "/v1/models", None, loopback, &[]).await;
    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(missing.body["error"]["type"], "authentication_error");
    assert_eq!(missing.body["error"]["code"], "unauthorized");

    let valid = request_json(
        app.clone(),
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[("authorization", format!("Bearer {first_token}"))],
    )
    .await;
    assert_eq!(valid.status, StatusCode::OK);
    assert_eq!(valid.body["object"], "list");
    assert_eq!(valid.body["data"].as_array().map(Vec::len), Some(0));

    let conflicting = request_json(
        app.clone(),
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[
            ("authorization", format!("Bearer {first_token}")),
            ("x-api-key", "a2k_v1_conflicting".to_owned()),
        ],
    )
    .await;
    assert_eq!(conflicting.status, StatusCode::BAD_REQUEST);
    assert_eq!(conflicting.body["error"]["code"], "invalid_request");

    let rotated = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/gateway-api-keys/{key_id}/rotate"),
        Some(json!({
            "expected_revision": 2,
            "expected_config_version": 1,
            "expected_token_version": 1
        })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK);
    let second_token = rotated.body["token"]
        .as_str()
        .expect("rotated token")
        .to_owned();
    assert_ne!(first_token, second_token);
    assert_eq!(rotated.body["items"][0]["token_version"], 2);

    let old = request_json(
        app.clone(),
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[("x-api-key", first_token.clone())],
    )
    .await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    let current = request_json(
        app.clone(),
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[("x-api-key", second_token.clone())],
    )
    .await;
    assert_eq!(current.status, StatusCode::OK);

    let revoked = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/gateway-api-keys/{key_id}/revoke"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 2
        })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(revoked.status, StatusCode::OK);
    assert_eq!(revoked.body["items"][0]["enabled"], false);

    let revoked_request = request_json(
        app.clone(),
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[("authorization", format!("Bearer {second_token}"))],
    )
    .await;
    assert_eq!(revoked_request.status, StatusCode::UNAUTHORIZED);

    let remote_admin = request_json(
        app,
        Method::GET,
        "/api/admin/gateway-api-keys",
        None,
        SocketAddr::from(([192, 0, 2, 10], 41000)),
        &[("x-api-key", "not-an-admin-token".to_owned())],
    )
    .await;
    assert_eq!(remote_admin.status, StatusCode::FORBIDDEN);
    assert_eq!(remote_admin.body["error"]["code"], "admin_loopback_only");
}

#[tokio::test]
async fn models_list_reflects_enabled_published_routes() {
    let (_directory, app) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let created_key = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({
            "expected_revision": 1,
            "name": "Models client",
            "enabled": true
        })),
        loopback,
        &[],
    )
    .await;
    let token = created_key.body["token"]
        .as_str()
        .expect("gateway token")
        .to_owned();

    let endpoint = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 2,
            "name": "Codex",
            "provider_kind": "codex",
            "base_url": "https://api.example.com",
            "protocol_dialect": "openai_responses",
            "allow_insecure_http": false,
            "allow_private_network": false,
            "enabled": true
        })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(endpoint.status, StatusCode::OK);
    let endpoint_id = endpoint.body["items"][0]["id"]
        .as_str()
        .expect("endpoint id")
        .to_owned();

    let route = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/model-routes",
        Some(json!({
            "expected_revision": 3,
            "public_model": "codex-local",
            "ingress_protocol": "openai_responses",
            "fallback_on_saturation": null,
            "enabled": true,
            "targets": [{
                "provider_endpoint_id": endpoint_id.clone(),
                "upstream_model": "gpt-5.1-codex",
                "fallback_tier": 0,
                "enabled": true
            }]
        })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(route.status, StatusCode::OK);
    let route_id = route.body["items"][0]["id"]
        .as_str()
        .expect("route id")
        .to_owned();
    let target_id = route.body["items"][0]["targets"][0]["id"]
        .as_str()
        .expect("target id")
        .to_owned();

    let listed = request_json(
        app.clone(),
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.body["object"], "list");
    assert_eq!(listed.body["data"][0]["id"], "codex-local");
    assert_eq!(listed.body["data"][0]["object"], "model");
    assert_eq!(listed.body["data"][0]["owned_by"], "any2api");

    let disabled = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(json!({
            "expected_revision": 4,
            "expected_config_version": 1,
            "public_model": "codex-local",
            "ingress_protocol": "openai_responses",
            "fallback_on_saturation": null,
            "enabled": false,
            "targets": [{
                "id": target_id,
                "provider_endpoint_id": endpoint_id.clone(),
                "upstream_model": "gpt-5.1-codex",
                "fallback_tier": 0,
                "enabled": true
            }]
        })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(disabled.status, StatusCode::OK);

    let listed = request_json(
        app,
        Method::GET,
        "/v1/models",
        None,
        loopback,
        &[("x-api-key", token)],
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.body["data"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn unknown_public_routes_never_fall_back_to_the_spa() {
    let (_directory, app) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let created = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({
            "expected_revision": 1,
            "name": "CLI",
            "enabled": true
        })),
        loopback,
        &[],
    )
    .await;
    let token = created.body["token"].as_str().expect("token").to_owned();
    let response = request_json(
        app.clone(),
        Method::GET,
        "/v1/not-a-route",
        None,
        loopback,
        &[("x-api-key", token.clone())],
    )
    .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(response.body["error"]["code"], "public_api_not_found");
    assert!(!response.raw_body.contains("any2api shell"));

    let namespace_root = request_json(
        app,
        Method::GET,
        "/v1/",
        None,
        loopback,
        &[("x-api-key", token)],
    )
    .await;
    assert_eq!(namespace_root.status, StatusCode::NOT_FOUND);
    assert_eq!(namespace_root.body["error"]["code"], "public_api_not_found");
    assert!(!namespace_root.raw_body.contains("any2api shell"));
}

async fn test_app() -> (tempfile::TempDir, Router) {
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
        storage,
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    ));
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    (
        directory,
        build_router(
            AppState::new(snapshots, runtime, publisher, public_requests),
            web_root,
        ),
    )
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
