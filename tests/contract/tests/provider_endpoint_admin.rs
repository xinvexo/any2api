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
async fn provider_endpoint_admin_is_loopback_only() {
    let (_directory, app, _storage) = test_app().await;
    let (status, body) = request_json(
        app,
        Method::GET,
        "/api/admin/provider-endpoints",
        None,
        SocketAddr::from(([203, 0, 113, 10], 41000)),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");
}

#[tokio::test]
async fn provider_protocol_options_and_optional_bridge_are_registry_driven() {
    let (_directory, app, _storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, initial) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/provider-endpoints",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        initial["protocol_options"]
            .as_array()
            .is_some_and(|options| {
                options.iter().any(|option| {
                    option["provider_kind"] == "codex"
                        && option["accepted_protocol"] == "openai_responses"
                        && option["upstream_protocols"]
                            .as_array()
                            .is_some_and(|protocols| {
                                protocols
                                    .iter()
                                    .any(|protocol| protocol == "openai_chat_completions")
                            })
                })
            })
    );

    let (status, bridged) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Responses through Chat",
            "provider_kind": "codex",
            "base_url": "https://chat-compatible.example.com/v1",
            "protocol_dialect": "openai_responses",
            "upstream_protocol_dialect": "openai_chat_completions",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        bridged["items"][0]["upstream_protocol_dialect"],
        "openai_chat_completions"
    );

    let (status, direct_chat) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 2,
            "name": "Direct Chat",
            "provider_kind": "codex",
            "base_url": "https://chat.example.com/v1",
            "protocol_dialect": "openai_chat_completions",
            "upstream_protocol_dialect": "openai_chat_completions",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let direct_chat = direct_chat["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["name"] == "Direct Chat")
        .expect("direct chat endpoint");
    assert_eq!(direct_chat["upstream_protocol_dialect"], Value::Null);

    let (status, unsupported) = request_json(
        app,
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 3,
            "name": "Unsupported",
            "provider_kind": "claude",
            "base_url": "https://api.example.com/v1",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(unsupported["error"]["code"], "invalid_provider_protocol");
}

#[tokio::test]
async fn provider_endpoint_crud_uses_base_url_as_network_authority_and_preserves_revision() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, initial) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/provider-endpoints",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["config_revision"], 1);
    assert_eq!(initial["items"].as_array().map(Vec::len), Some(0));

    let (status, _) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Legacy Flags",
            "provider_kind": "codex",
            "base_url": "http://127.0.0.1:8080/v1",
            "protocol_dialect": "openai_responses",
            "allow_private_network": true,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Codex Primary",
            "provider_kind": "codex",
            "base_url": "https://api.example.com/v1/",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["config_revision"], 2);
    assert_eq!(
        created["items"][0]["base_url"],
        "https://api.example.com/v1"
    );
    let created_endpoint = created["items"][0].as_object().expect("created endpoint");
    assert!(!created_endpoint.contains_key("allow_insecure_http"));
    assert!(!created_endpoint.contains_key("allow_private_network"));

    let (status, http_allowed) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 2,
            "name": "HTTP Public",
            "provider_kind": "codex",
            "base_url": "http://api.example.com",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(http_allowed["config_revision"], 3);

    let (status, private_allowed) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 3,
            "name": "Private",
            "provider_kind": "claude",
            "base_url": "https://127.0.0.1:8443",
            "protocol_dialect": "anthropic_messages",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(private_allowed["config_revision"], 4);

    let (status, private_http_allowed) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 4,
            "name": "Private HTTP",
            "provider_kind": "claude",
            "base_url": "http://127.0.0.1:8443",
            "protocol_dialect": "anthropic_messages",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(private_http_allowed["config_revision"], 5);
    let private_id = private_http_allowed["items"]
        .as_array()
        .expect("items")
        .iter()
        .find(|item| item["name"] == "Private HTTP")
        .and_then(|item| item["id"].as_str())
        .expect("private endpoint id")
        .to_owned();

    let (status, incompatible) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/provider-endpoints/{private_id}"),
        Some(json!({
            "expected_revision": 5,
            "expected_config_version": 1,
            "name": "Private HTTP",
            "provider_kind": "claude",
            "base_url": "http://127.0.0.1:8443",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(incompatible["error"]["code"], "invalid_provider_protocol");

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/provider-endpoints/{private_id}"),
        Some(json!({
            "expected_revision": 5,
            "expected_config_version": 1,
            "name": "Private HTTP Updated",
            "provider_kind": "claude",
            "base_url": "http://127.0.0.1:8443",
            "protocol_dialect": "anthropic_messages",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 6);

    let (status, endpoint_stale) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/provider-endpoints/{private_id}"),
        Some(json!({
            "expected_revision": 6,
            "expected_config_version": 1,
            "name": "Stale Draft",
            "provider_kind": "claude",
            "base_url": "http://127.0.0.1:8443",
            "protocol_dialect": "anthropic_messages",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        endpoint_stale["error"]["code"],
        "provider_endpoint_version_conflict"
    );

    let (status, stale) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/provider-endpoints/{private_id}?expected_revision=1"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["error"]["code"], "revision_conflict");

    let (status, deleted) = request_json(
        app,
        Method::DELETE,
        &format!("/api/admin/provider-endpoints/{private_id}?expected_revision=6"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["config_revision"], 7);
    assert_eq!(deleted["items"].as_array().map(Vec::len), Some(3));

    let stored = storage.load_configuration().await.expect("configuration");
    assert_eq!(stored.revision().get(), 7);
    assert_eq!(stored.provider_endpoints().endpoints().len(), 3);
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
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            any2api_contract_tests::build_configuration_capabilities(),
        )
        .expect("configuration publisher"),
    );
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, public_requests),
        web_root,
    );
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
