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

#[tokio::test]
async fn model_route_admin_publishes_atomic_same_protocol_aggregates() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let codex = create_endpoint(app.clone(), loopback, 1, "Codex", "codex").await;
    let codex_id = codex["items"][0]["id"]
        .as_str()
        .expect("codex id")
        .to_owned();
    let claude = create_endpoint(app.clone(), loopback, 2, "Claude", "claude").await;
    let claude_id = claude["items"]
        .as_array()
        .expect("endpoints")
        .iter()
        .find(|endpoint| endpoint["provider_kind"] == "claude")
        .and_then(|endpoint| endpoint["id"].as_str())
        .expect("claude id")
        .to_owned();

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/model-routes",
        Some(route_body(
            3,
            None,
            "codex",
            "openai_responses",
            &codex_id,
            None,
            "gpt-5.1",
            0,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["config_revision"], 4);
    assert_eq!(created["items"][0]["config_version"], 1);
    assert_eq!(created["items"][0]["fallback_on_saturation"], Value::Null);
    let route_id = created["items"][0]["id"]
        .as_str()
        .expect("route id")
        .to_owned();
    let target_id = created["items"][0]["targets"][0]["id"]
        .as_str()
        .expect("target id")
        .to_owned();

    let (status, cross_protocol) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/model-routes",
        Some(route_body(
            4,
            None,
            "cross",
            "openai_responses",
            &claude_id,
            None,
            "claude-sonnet",
            0,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(cross_protocol["error"]["code"], "invalid_model_route");

    let (status, missing_existing_id) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(route_body(
            4,
            Some(1),
            "codex",
            "openai_responses",
            &codex_id,
            None,
            "gpt-5.1",
            0,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(missing_existing_id["error"]["code"], "invalid_request");

    let (status, unchanged) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/model-routes",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unchanged["config_revision"], 4);
    assert_eq!(unchanged["items"][0]["config_version"], 1);
    assert_eq!(unchanged["items"][0]["targets"][0]["id"], target_id);

    let (status, unknown_target) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(route_body(
            4,
            Some(1),
            "codex",
            "openai_responses",
            &codex_id,
            Some("4b8db7ee-74d3-487c-a46b-c95403e1ce72"),
            "gpt-5.1",
            0,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(unknown_target["error"]["code"], "invalid_request");

    let (status, stale_revision) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(route_body(
            3,
            Some(1),
            "codex",
            "openai_responses",
            &codex_id,
            Some(&target_id),
            "gpt-5.1",
            0,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale_revision["error"]["code"], "revision_conflict");

    let (status, identity_change) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(route_body(
            4,
            Some(1),
            "codex",
            "openai_responses",
            &codex_id,
            Some(&target_id),
            "different-upstream",
            0,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        identity_change["error"]["code"],
        "route_target_identity_conflict"
    );

    let mut update = route_body(
        4,
        Some(1),
        "codex-local",
        "openai_responses",
        &codex_id,
        Some(&target_id),
        "gpt-5.1",
        2,
    );
    update["fallback_on_saturation"] = Value::Bool(true);
    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(update),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 5);
    assert_eq!(updated["items"][0]["config_version"], 2);
    assert_eq!(updated["items"][0]["targets"][0]["id"], target_id);
    assert_eq!(updated["items"][0]["targets"][0]["fallback_tier"], 2);

    let (status, stale_route_version) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/model-routes/{route_id}"),
        Some(route_body(
            5,
            Some(1),
            "stale-route",
            "openai_responses",
            &codex_id,
            Some(&target_id),
            "gpt-5.1",
            2,
        )),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        stale_route_version["error"]["code"],
        "model_route_version_conflict"
    );

    let (status, endpoint_identity_in_use) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/provider-endpoints/{codex_id}"),
        Some(json!({
            "expected_revision": 5,
            "expected_config_version": 1,
            "name": "Codex",
            "provider_kind": "claude",
            "base_url": "https://api.example.com",
            "protocol_dialect": "anthropic_messages",
            "allow_insecure_http": false,
            "allow_private_network": false,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        endpoint_identity_in_use["error"]["code"],
        "provider_endpoint_identity_in_use"
    );

    let (status, endpoint_in_use) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/provider-endpoints/{codex_id}?expected_revision=5"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(endpoint_in_use["error"]["code"], "provider_endpoint_in_use");

    let (status, deleted) = request_json(
        app,
        Method::DELETE,
        &format!(
            "/api/admin/model-routes/{route_id}?expected_revision=5&expected_config_version=2"
        ),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["config_revision"], 6);
    assert_eq!(deleted["items"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        storage
            .load_configuration()
            .await
            .expect("configuration")
            .model_routes()
            .routes()
            .len(),
        0
    );
}

#[tokio::test]
async fn model_route_admin_is_loopback_only() {
    let (_directory, app, _storage) = test_app().await;
    let (status, body) = request_json(
        app,
        Method::GET,
        "/api/admin/model-routes",
        None,
        SocketAddr::from(([203, 0, 113, 8], 41000)),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");
}

async fn create_endpoint(
    app: Router,
    remote: SocketAddr,
    expected_revision: u64,
    name: &str,
    provider_kind: &str,
) -> Value {
    let (status, body) = request_json(
        app,
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": expected_revision,
            "name": name,
            "provider_kind": provider_kind,
            "base_url": "https://api.example.com",
            "protocol_dialect": if provider_kind == "codex" {
                "openai_responses"
            } else {
                "anthropic_messages"
            },
            "allow_insecure_http": false,
            "allow_private_network": false,
            "enabled": true
        })),
        remote,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    body
}

#[allow(clippy::too_many_arguments)]
fn route_body(
    expected_revision: u64,
    expected_config_version: Option<u64>,
    public_model: &str,
    ingress_protocol: &str,
    endpoint_id: &str,
    target_id: Option<&str>,
    upstream_model: &str,
    fallback_tier: u16,
) -> Value {
    json!({
        "expected_revision": expected_revision,
        "expected_config_version": expected_config_version,
        "public_model": public_model,
        "ingress_protocol": ingress_protocol,
        "fallback_on_saturation": null,
        "enabled": true,
        "targets": [{
            "id": target_id,
            "provider_endpoint_id": endpoint_id,
            "upstream_model": upstream_model,
            "fallback_tier": fallback_tier,
            "enabled": true
        }]
    })
}

async fn test_app() -> (tempfile::TempDir, Router, Arc<SqliteStore>) {
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
