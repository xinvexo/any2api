use std::{net::SocketAddr, sync::Arc};

use any2api_contract_tests::TestApplication;
use any2api_domain::{RateLimitMode, SettingKey};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
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
async fn settings_admin_requires_a_session_for_remote_requests() {
    let (_directory, app, _storage) = test_app().await;
    let (status, body) = request_json(
        app,
        Method::GET,
        "/api/admin/settings",
        None,
        SocketAddr::from(([203, 0, 113, 10], 41000)),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "admin_session_required");
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
    assert_eq!(
        initial["items"].as_array().map(Vec::len),
        Some(SettingKey::ALL.len())
    );
    let remote = find_setting(&initial, "admin.remote_enabled");
    assert_eq!(remote["default_value"], true);
    assert_eq!(remote["effective_value"], true);
    assert_eq!(remote["web_group"], "远程管理");
    let trusted_proxies = find_setting(&initial, "network.trusted_proxy_cidrs");
    assert_eq!(trusted_proxies["value_type"], "string_list");
    assert_eq!(trusted_proxies["default_value"], json!([]));
    assert_eq!(trusted_proxies["override_value"], Value::Null);
    let models = find_setting(&initial, "models.allowed");
    assert_eq!(models["value_type"], "model_access");
    assert_eq!(models["default_value"], "all");
    let (status, invalid_models) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/models.allowed",
        Some(json!({ "expected_revision": 1, "value": null })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_models["error"]["code"], "invalid_setting");
    let timeout = find_setting(&initial, "scheduler.queue_timeout");
    assert_eq!(timeout["value_type"], "duration_secs");
    assert_eq!(timeout["default_value"], 180);
    assert_eq!(timeout["override_value"], Value::Null);
    assert_eq!(timeout["effective_value"], 180);
    assert_eq!(timeout["min_value"], 1);
    assert_eq!(timeout["max_value"], 86_400);
    assert_eq!(timeout["apply_mode"], "hot_reload");
    let retry_budget = find_setting(&initial, "retry.precommit_total_budget");
    assert_eq!(retry_budget["value_type"], "duration_secs");
    assert_eq!(retry_budget["default_value"], 600);
    assert!(
        initial["items"]
            .as_array()
            .expect("setting items")
            .iter()
            .all(|item| item["key"] != "retry.max_total_attempts")
    );
    let file_level = find_setting(&initial, "logs.file.level");
    assert_eq!(file_level["value_type"], "enum");
    assert_eq!(file_level["default_value"], "info");
    assert_eq!(
        file_level["allowed_values"],
        json!(["error", "warn", "info", "debug", "trace"])
    );

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/scheduler.on_rate_limited",
        Some(json!({ "expected_revision": 1, "value": "reject" })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 2);
    let rate_limited = find_setting(&updated, "scheduler.on_rate_limited");
    assert_eq!(rate_limited["allowed_values"], json!(["wait", "reject"]));
    assert_eq!(rate_limited["default_value"], "wait");
    assert_eq!(rate_limited["override_value"], "reject");
    assert_eq!(rate_limited["effective_value"], "reject");

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

    let (status, removed) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings/retry.max_total_attempts",
        Some(json!({ "expected_revision": 2, "value": 3 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(removed["error"]["code"], "setting_not_found");

    let (status, reset) = request_json(
        app,
        Method::DELETE,
        "/api/admin/settings/scheduler.on_rate_limited?expected_revision=2",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reset["config_revision"], 3);
    let rate_limited = find_setting(&reset, "scheduler.on_rate_limited");
    assert_eq!(rate_limited["override_value"], Value::Null);
    assert_eq!(rate_limited["effective_value"], "wait");

    let stored = storage.load_configuration().await.expect("stored settings");
    assert_eq!(stored.revision().get(), 3);
    assert_eq!(
        stored.settings().scheduler().on_rate_limited(),
        RateLimitMode::Wait
    );
    assert_eq!(
        stored
            .settings()
            .override_value(SettingKey::SchedulerOnRateLimited),
        None
    );
}

#[tokio::test]
async fn settings_batch_is_atomic_and_publishes_one_revision() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings",
        Some(json!({
            "expected_revision": 1,
            "updates": [
                { "key": "oauth.refresh.scan_interval", "value": 600 },
                { "key": "oauth.refresh.lead_time", "value": 900 }
            ],
            "resets": []
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 2);
    assert_eq!(
        find_setting(&updated, "oauth.refresh.scan_interval")["effective_value"],
        600
    );
    assert_eq!(
        find_setting(&updated, "oauth.refresh.lead_time")["effective_value"],
        900
    );

    let (status, invalid) = request_json(
        app.clone(),
        Method::PATCH,
        "/api/admin/settings",
        Some(json!({
            "expected_revision": 2,
            "updates": [
                { "key": "oauth.refresh.scan_interval", "value": 1_000 },
                { "key": "oauth.refresh.lead_time", "value": 500 }
            ],
            "resets": []
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid["error"]["code"], "invalid_setting");
    assert_eq!(
        storage
            .load_configuration()
            .await
            .expect("unchanged settings")
            .revision()
            .get(),
        2
    );

    let (status, reset) = request_json(
        app,
        Method::PATCH,
        "/api/admin/settings",
        Some(json!({
            "expected_revision": 2,
            "updates": [],
            "resets": ["oauth.refresh.scan_interval", "oauth.refresh.lead_time"]
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reset["config_revision"], 3);
    assert_eq!(
        find_setting(&reset, "oauth.refresh.scan_interval")["effective_value"],
        30
    );
    assert_eq!(
        find_setting(&reset, "oauth.refresh.lead_time")["effective_value"],
        300
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
    TestApplication::new().await.into_router()
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
