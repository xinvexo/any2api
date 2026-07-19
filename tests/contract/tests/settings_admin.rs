use std::{fs, net::SocketAddr, sync::Arc};

use any2api_domain::{SaturationMode, SettingKey};
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
async fn settings_admin_is_loopback_only() {
    let (_directory, app, _storage) = test_app().await;
    let (status, body) = request_json(
        app,
        Method::GET,
        "/api/admin/settings",
        None,
        SocketAddr::from(([203, 0, 113, 10], 41000)),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");
}

#[tokio::test]
async fn settings_api_exposes_defaults_overrides_and_effective_values() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, initial) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/settings",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["config_revision"], 1);
    assert_eq!(initial["items"].as_array().map(Vec::len), Some(6));
    let timeout = find_setting(&initial, "scheduler.queue_timeout");
    assert_eq!(timeout["value_type"], "duration_ms");
    assert_eq!(timeout["default_value"], 30_000);
    assert_eq!(timeout["override_value"], Value::Null);
    assert_eq!(timeout["effective_value"], 30_000);
    assert_eq!(timeout["min_value"], 1);
    assert_eq!(timeout["max_value"], 86_400_000);
    assert_eq!(timeout["allowed_values"], Value::Null);
    assert_eq!(timeout["apply_mode"], "hot_reload");
    assert_eq!(timeout["web_group"], "排队策略");
    assert!(
        timeout["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/scheduler.on_saturated",
        Some(json!({ "expected_revision": 1, "value": "reject" })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 2);
    let saturated = find_setting(&updated, "scheduler.on_saturated");
    assert_eq!(saturated["allowed_values"], json!(["wait", "reject"]));
    assert_eq!(saturated["default_value"], "wait");
    assert_eq!(saturated["override_value"], "reject");
    assert_eq!(saturated["effective_value"], "reject");

    let (status, invalid) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/scheduler.queue_timeout",
        Some(json!({ "expected_revision": 2, "value": 0 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid["error"]["code"], "invalid_setting");

    let (status, stale) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/scheduler.max_waiting_requests",
        Some(json!({ "expected_revision": 1, "value": 64 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["error"]["code"], "revision_conflict");

    let (status, missing) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/scheduler.unknown",
        Some(json!({ "expected_revision": 2, "value": true })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["error"]["code"], "setting_not_found");

    let (status, reset) = request_json(
        app,
        Method::DELETE,
        "/api/admin/settings/scheduler.on_saturated?expected_revision=2",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reset["config_revision"], 3);
    let saturated = find_setting(&reset, "scheduler.on_saturated");
    assert_eq!(saturated["override_value"], Value::Null);
    assert_eq!(saturated["effective_value"], "wait");

    let stored = storage.load_configuration().await.expect("stored settings");
    assert_eq!(stored.revision().get(), 3);
    assert_eq!(
        stored.settings().scheduler().on_saturated(),
        SaturationMode::Wait
    );
    assert_eq!(
        stored
            .settings()
            .override_value(SettingKey::SchedulerOnSaturated),
        None
    );
}

fn find_setting<'a>(response: &'a Value, key: &str) -> &'a Value {
    response["items"]
        .as_array()
        .expect("setting items")
        .iter()
        .find(|item| item["key"] == key)
        .expect("setting item")
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
