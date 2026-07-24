use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MaxConcurrency, OAuthAccountDraft, OAuthAccountId,
    ProtocolDialect, ProtocolOperation, ProviderKind, ProxyProfileId, RequestId, RequestLog,
};
use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{
    ConfigurationRepository, OAuthAccountDocument, RequestLogRepository, SqliteStore,
};
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
async fn oauth_account_admin_crud_is_safe_and_revisioned() {
    let (_directory, app, storage, account_id) = test_app().await;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    storage
        .append_request_logs(&[
            oauth_request_log(account_id, now_ms.saturating_sub(1_000), 200),
            oauth_request_log(account_id, now_ms, 503),
        ])
        .await
        .expect("append OAuth usage");
    let remote = SocketAddr::from(([203, 0, 113, 10], 41000));
    let (status, forbidden) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/oauth/accounts",
        None,
        remote,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(forbidden["error"]["code"], "admin_loopback_only");

    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, listed) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/oauth/accounts",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["config_revision"], 2);
    assert_eq!(listed["items"].as_array().map(Vec::len), Some(1));
    let account = &listed["items"][0];
    assert_eq!(account["id"], account_id.to_string());
    assert_eq!(account["provider_kind"], "codex");
    assert_eq!(account["label"], "Primary Codex OAuth");
    assert_eq!(account["max_concurrency"], 1);
    assert_eq!(account["enabled"], true);
    assert_eq!(account["safe_account_email"], "person@example.com");
    assert_eq!(account["token_version"], 1);
    assert_eq!(account["account_generation"], 1);
    assert_eq!(account["config_version"], 1);
    assert_eq!(account["models"], json!(["gpt-5.5"]));
    assert_eq!(
        account["available_models"],
        json!([
            "codex-auto-review",
            "gpt-5.4-mini",
            "gpt-5.5",
            "gpt-5.6-luna",
            "gpt-5.6-terra"
        ])
    );
    // Test fixture token has no id_token plan claim.
    assert_eq!(account["plan_type"], Value::Null);
    assert_eq!(account["usage"]["total_requests"], 2);
    assert_eq!(account["usage"]["successful_requests"], 1);
    assert_eq!(account["usage"]["failed_requests"], 1);
    assert_eq!(account["usage"]["window_minutes"], 2);
    let slots = account["usage"]["window_slots"]
        .as_array()
        .expect("window slots");
    assert_eq!(slots.len(), 30);
    let newest = slots.last().expect("newest slot");
    assert_eq!(newest["total_requests"], 2);
    assert_eq!(newest["successful_requests"], 1);
    assert_eq!(newest["failed_requests"], 1);
    let listed_text = serde_json::to_string(&listed).expect("listed JSON");
    assert!(!listed_text.contains("access-secret"));
    assert!(!listed_text.contains("refresh-secret"));
    assert!(!listed_text.contains("oauth_json"));

    let (status, rejected) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/oauth/accounts/{account_id}"),
        Some(json!({
            "expected_revision": 2,
            "expected_config_version": 1,
            "label": "Renamed OAuth",
            "max_concurrency": 3,
            "enabled": false,
            "oauth_json": {"access_token": "replacement"}
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(rejected["error"]["code"], "invalid_request");

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/oauth/accounts/{account_id}"),
        Some(json!({
            "expected_revision": 2,
            "expected_config_version": 1,
            "label": "Renamed OAuth",
            "max_concurrency": 3,
            "enabled": false
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 3);
    assert_eq!(updated["items"][0]["label"], "Renamed OAuth");
    assert_eq!(updated["items"][0]["max_concurrency"], 3);
    assert_eq!(updated["items"][0]["enabled"], false);
    assert_eq!(updated["items"][0]["config_version"], 2);
    assert_eq!(updated["items"][0]["account_generation"], 1);

    let (status, unavailable) = request_json(
        app.clone(),
        Method::PUT,
        &format!("/api/admin/oauth/accounts/{account_id}/models"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 2,
            "models": ["gpt-not-in-plan"]
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(unavailable["error"]["code"], "oauth_model_unavailable");

    let (status, models) = request_json(
        app.clone(),
        Method::PUT,
        &format!("/api/admin/oauth/accounts/{account_id}/models"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 2,
            "models": ["gpt-5.5", "gpt-5.6-luna"]
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(models["config_revision"], 4);
    assert_eq!(models["items"][0]["config_version"], 3);
    assert_eq!(models["items"][0]["selected_model_count"], 2);
    assert_eq!(
        models["items"][0]["models"],
        json!(["gpt-5.5", "gpt-5.6-luna"])
    );

    let (status, stale) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/oauth/accounts/{account_id}"),
        Some(json!({
            "expected_revision": 4,
            "expected_config_version": 2,
            "label": "Stale OAuth",
            "max_concurrency": 1,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["error"]["code"], "oauth_account_version_conflict");

    let (status, deleted) = request_json(
        app,
        Method::DELETE,
        &format!(
            "/api/admin/oauth/accounts/{account_id}?expected_revision=4&expected_config_version=3"
        ),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["config_revision"], 5);
    assert_eq!(deleted["items"], json!([]));

    let stored = storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 5);
    assert!(stored.oauth_accounts().accounts().is_empty());
    assert!(
        stored
            .into_parts()
            .oauth_account_materials
            .into_entries()
            .is_empty()
    );
}

async fn test_app() -> (tempfile::TempDir, Router, Arc<SqliteStore>, OAuthAccountId) {
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
    let account_id = OAuthAccountId::new();
    publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            OAuthAccountDraft::new(
                "Primary Codex OAuth",
                MaxConcurrency::new(1).expect("max concurrency"),
                true,
            )
            .expect("OAuth account draft"),
            Some("person@example.com".to_owned()),
            Some(1_800_000_000),
            vec!["gpt-5.5".to_owned()],
            OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"type":"codex","access_token":"access-secret","refresh_token":"refresh-secret"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth account document"),
        )
        .await
        .expect("activate OAuth account");
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let components = build_public_request_components().expect("public request components");
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        snapshots.load().revision(),
        snapshots.load().settings().logging(),
        &runtime.lifecycle(),
    ));
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, components.service())
            .with_request_telemetry(telemetry),
        web_root,
    );
    (directory, app, storage, account_id)
}

fn oauth_request_log(
    account_id: OAuthAccountId,
    started_at_ms: u64,
    status_code: u16,
) -> CompletedRequestLog {
    CompletedRequestLog {
        request: RequestLog {
            request_id: RequestId::new(),
            started_at_ms,
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("gpt-5.5".into()),
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: Some(account_id),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code,
            error_class: None,
            attempt_count: 0,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            is_stream: false,
        },
        attempts: Vec::new(),
    }
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
        Body::from(serde_json::to_vec(&value).expect("request JSON"))
    } else {
        Body::empty()
    };
    let response = app
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let value = serde_json::from_slice(
        &response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes(),
    )
    .expect("response JSON");
    (status, value)
}
