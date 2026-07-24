use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{FileLogLevel, SaturationMode, SettingKey};
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
    assert_eq!(initial["items"].as_array().map(Vec::len), Some(51));
    let remote = find_setting(&initial, "admin.remote_enabled");
    assert_eq!(remote["default_value"], false);
    assert_eq!(remote["effective_value"], false);
    assert_eq!(remote["web_group"], "远程管理");
    let soft_mode = find_setting(&initial, "affinity.soft.mode");
    assert_eq!(soft_mode["value_type"], "enum");
    assert_eq!(soft_mode["default_value"], "prefer");
    assert_eq!(soft_mode["effective_value"], "prefer");
    assert_eq!(soft_mode["allowed_values"], json!(["prefer", "strict"]));
    let hard_ttl = find_setting(&initial, "affinity.hard.ttl");
    assert_eq!(hard_ttl["default_value"], 86_400);
    assert_eq!(hard_ttl["min_value"], 1);
    let timeout = find_setting(&initial, "scheduler.queue_timeout");
    assert_eq!(timeout["value_type"], "duration_secs");
    assert_eq!(timeout["default_value"], 30);
    assert_eq!(timeout["override_value"], Value::Null);
    assert_eq!(timeout["effective_value"], 30);
    assert_eq!(timeout["min_value"], 1);
    assert_eq!(timeout["max_value"], 86_400);
    assert_eq!(timeout["allowed_values"], Value::Null);
    assert_eq!(timeout["apply_mode"], "hot_reload");
    assert_eq!(timeout["web_group"], "排队策略");
    assert!(
        timeout["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    let attempts = find_setting(&initial, "retry.max_total_attempts");
    assert_eq!(attempts["default_value"], 3);
    assert_eq!(attempts["min_value"], 1);
    assert_eq!(attempts["max_value"], 10);
    assert_eq!(attempts["web_group"], "重试预算");
    let endpoint_window = find_setting(&initial, "breaker.endpoint.failure_window");
    assert_eq!(endpoint_window["value_type"], "duration_secs");
    assert_eq!(endpoint_window["default_value"], 30);
    let stream_bytes = find_setting(&initial, "stream.precommit.max_bytes");
    assert_eq!(stream_bytes["value_type"], "integer");
    assert_eq!(stream_bytes["default_value"], 256 * 1024);
    assert_eq!(stream_bytes["min_value"], 1);
    assert_eq!(stream_bytes["max_value"], 16 * 1024 * 1024);
    assert_eq!(stream_bytes["web_group"], "流式预提交");
    assert!(
        stream_bytes["description"]
            .as_str()
            .is_some_and(|value| value.contains("每个 SSE 帧"))
    );
    let stream_duration = find_setting(&initial, "stream.precommit.max_duration");
    assert_eq!(stream_duration["value_type"], "duration_secs");
    assert_eq!(stream_duration["default_value"], 5);
    assert_eq!(stream_duration["min_value"], 1);
    assert_eq!(stream_duration["max_value"], 86_400);
    let read_timeout = find_setting(&initial, "upstream.read_timeout");
    assert_eq!(read_timeout["value_type"], "duration_secs");
    assert_eq!(read_timeout["default_value"], 15);
    assert_eq!(read_timeout["min_value"], 1);
    assert_eq!(read_timeout["max_value"], 86_400);
    assert_eq!(read_timeout["web_group"], "上游网络");
    let refresh_scan = find_setting(&initial, "oauth.refresh.scan_interval");
    assert_eq!(refresh_scan["default_value"], 30);
    assert_eq!(refresh_scan["min_value"], 1);
    assert_eq!(refresh_scan["max_value"], 86_400);
    assert_eq!(refresh_scan["web_group"], "OAuth 刷新");
    let refresh_lead = find_setting(&initial, "oauth.refresh.lead_time");
    assert_eq!(refresh_lead["default_value"], 300);
    assert_eq!(refresh_lead["min_value"], 1);
    assert_eq!(refresh_lead["max_value"], 86_400);
    let strict_ssrf = find_setting(&initial, "upstream.strict_ssrf");
    assert_eq!(strict_ssrf["value_type"], "boolean");
    assert_eq!(strict_ssrf["default_value"], false);
    assert_eq!(strict_ssrf["effective_value"], false);
    let postcommit_idle = find_setting(&initial, "stream.postcommit.idle_timeout");
    assert_eq!(postcommit_idle["value_type"], "duration_secs");
    assert_eq!(postcommit_idle["default_value"], 60);
    assert_eq!(postcommit_idle["min_value"], 1);
    assert_eq!(postcommit_idle["max_value"], 86_400);
    assert_eq!(postcommit_idle["web_group"], "流式响应");
    let shutdown_grace = find_setting(&initial, "shutdown.request_grace_period");
    assert_eq!(shutdown_grace["default_value"], 30);
    assert_eq!(shutdown_grace["min_value"], 1);
    assert_eq!(shutdown_grace["max_value"], 300);
    assert_eq!(shutdown_grace["web_group"], "优雅停机");
    let shutdown_finalize = find_setting(&initial, "shutdown.finalize_timeout");
    assert_eq!(shutdown_finalize["default_value"], 5);
    assert_eq!(shutdown_finalize["min_value"], 1);
    assert_eq!(shutdown_finalize["max_value"], 60);
    let file_level = find_setting(&initial, "logs.file.level");
    assert_eq!(file_level["value_type"], "enum");
    assert_eq!(file_level["default_value"], "info");
    assert_eq!(
        file_level["allowed_values"],
        json!(["error", "warn", "info", "debug", "trace"])
    );
    assert_eq!(file_level["web_group"], "本地文件日志");
    let file_retention = find_setting(&initial, "logs.file.retention");
    assert_eq!(file_retention["default_value"], 604_800);
    assert_eq!(file_retention["min_value"], 60);
    let file_size = find_setting(&initial, "logs.file.max_total_size");
    assert_eq!(file_size["default_value"], 256 * 1024 * 1024);
    assert_eq!(file_size["min_value"], 1024 * 1024);
    assert!(
        initial["items"]
            .as_array()
            .expect("setting items")
            .iter()
            .all(|item| item["key"] != "stream.precommit.max_events")
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

#[tokio::test]
async fn shutdown_settings_publish_and_restore_defaults() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/shutdown.request_grace_period",
        Some(json!({ "expected_revision": 1, "value": 45 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 2);
    assert_eq!(
        find_setting(&updated, "shutdown.request_grace_period")["effective_value"],
        45
    );
    assert_eq!(
        storage
            .load_configuration()
            .await
            .expect("stored settings")
            .settings()
            .shutdown()
            .request_grace_period_secs(),
        45
    );

    let (status, reset) = request_json(
        app,
        Method::DELETE,
        "/api/admin/settings/shutdown.request_grace_period?expected_revision=2",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reset["config_revision"], 3);
    assert_eq!(
        find_setting(&reset, "shutdown.request_grace_period")["effective_value"],
        30
    );
}

#[tokio::test]
async fn oauth_refresh_settings_publish_validate_and_restore_defaults() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, scan) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/oauth.refresh.scan_interval",
        Some(json!({ "expected_revision": 1, "value": 60 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scan["config_revision"], 2);
    assert_eq!(
        find_setting(&scan, "oauth.refresh.scan_interval")["effective_value"],
        60
    );

    let (status, invalid) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/oauth.refresh.lead_time",
        Some(json!({ "expected_revision": 2, "value": 30 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid["error"]["code"], "invalid_setting");

    let (status, lead) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/oauth.refresh.lead_time",
        Some(json!({ "expected_revision": 2, "value": 120 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(lead["config_revision"], 3);
    assert_eq!(
        storage
            .load_configuration()
            .await
            .expect("stored settings")
            .settings()
            .oauth()
            .refresh_lead_time_secs(),
        120
    );

    let (status, reset_scan) = request_json(
        app.clone(),
        Method::DELETE,
        "/api/admin/settings/oauth.refresh.scan_interval?expected_revision=3",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find_setting(&reset_scan, "oauth.refresh.scan_interval")["effective_value"],
        30
    );

    let (status, reset_lead) = request_json(
        app,
        Method::DELETE,
        "/api/admin/settings/oauth.refresh.lead_time?expected_revision=4",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find_setting(&reset_lead, "oauth.refresh.lead_time")["effective_value"],
        300
    );
}

#[tokio::test]
async fn file_log_settings_publish_and_restore_defaults() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, level) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/logs.file.level",
        Some(json!({ "expected_revision": 1, "value": "debug" })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find_setting(&level, "logs.file.level")["effective_value"],
        "debug"
    );

    let (status, retention) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/logs.file.retention",
        Some(json!({ "expected_revision": 2, "value": 120 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find_setting(&retention, "logs.file.retention")["effective_value"],
        120
    );

    let (status, size) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/logs.file.max_total_size",
        Some(json!({ "expected_revision": 3, "value": 2 * 1024 * 1024 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        find_setting(&size, "logs.file.max_total_size")["effective_value"],
        2 * 1024 * 1024
    );

    let stored = storage.load_configuration().await.expect("stored settings");
    assert_eq!(
        stored.settings().logging().file_level(),
        FileLogLevel::Debug
    );
    assert_eq!(stored.settings().logging().file_retention_secs(), 120);
    assert_eq!(
        stored.settings().logging().file_max_total_size(),
        2 * 1024 * 1024
    );

    let mut revision = 4;
    for key in [
        "logs.file.level",
        "logs.file.retention",
        "logs.file.max_total_size",
    ] {
        let (status, reset) = request_json(
            app.clone(),
            Method::DELETE,
            &format!("/api/admin/settings/{key}?expected_revision={revision}"),
            None,
            loopback,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(find_setting(&reset, key)["override_value"], Value::Null);
        revision += 1;
    }

    let stored = storage.load_configuration().await.expect("reset settings");
    assert_eq!(stored.settings().logging().file_level(), FileLogLevel::Info);
    assert_eq!(stored.settings().logging().file_retention_secs(), 604_800);
    assert_eq!(
        stored.settings().logging().file_max_total_size(),
        256 * 1024 * 1024
    );
}

#[tokio::test]
async fn strict_ssrf_setting_publishes_and_restores_the_default() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, enabled) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/upstream.strict_ssrf",
        Some(json!({ "expected_revision": 1, "value": true })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(enabled["config_revision"], 2);
    assert_eq!(
        find_setting(&enabled, "upstream.strict_ssrf")["effective_value"],
        true
    );
    assert!(
        storage
            .load_configuration()
            .await
            .expect("stored settings")
            .settings()
            .upstream()
            .strict_ssrf()
    );

    let (status, reset) = request_json(
        app,
        Method::DELETE,
        "/api/admin/settings/upstream.strict_ssrf?expected_revision=2",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reset["config_revision"], 3);
    assert_eq!(
        find_setting(&reset, "upstream.strict_ssrf")["effective_value"],
        false
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
