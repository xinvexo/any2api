use std::{net::SocketAddr, sync::Arc};

use any2api_contract_tests::TestApplication;
use any2api_domain::{
    CompletedRequestLog, ConfigRevision, MAX_REQUEST_LOG_ROWS, OAuthAccountDraft, OAuthAccountId,
    OAuthProxySelection, ProtocolDialect, ProtocolOperation, ProviderKind, ProxyProfileId,
    RequestId, RequestLog,
};
use any2api_runtime::api::{OAuthService, RequestTelemetry};
use any2api_storage::api::{
    ConfigurationRepository, OAuthAccountDocument, OAuthModelCatalogSnapshotRepository,
    RequestLogRepository, SqliteStore, StoredOAuthModelCatalogSnapshot,
};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::Connection;
use tower::ServiceExt;

#[tokio::test]
async fn oauth_account_admin_crud_is_safe_and_revisioned() {
    let (_directory, app, storage, account_id) = test_app().await;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64;
    storage
        .append_request_logs(
            &[
                oauth_request_log(account_id, now_ms.saturating_sub(1_000), 200),
                oauth_request_log(account_id, now_ms, 503),
            ],
            MAX_REQUEST_LOG_ROWS,
        )
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
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(forbidden["error"]["code"], "admin_session_required");

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
    assert_eq!(account["requests_per_minute"], Value::Null);
    assert_eq!(account["enabled"], true);
    assert_eq!(account["proxy_selection"]["mode"], "global");
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
    let window_totals = slots.iter().fold((0, 0, 0), |totals, slot| {
        (
            totals.0 + slot["total_requests"].as_u64().expect("slot total"),
            totals.1
                + slot["successful_requests"]
                    .as_u64()
                    .expect("slot successes"),
            totals.2 + slot["failed_requests"].as_u64().expect("slot failures"),
        )
    });
    assert_eq!(window_totals, (2, 1, 1));
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
            "requests_per_minute": 3,
            "enabled": false,
            "proxy_selection": {"mode": "global"},
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
            "requests_per_minute": 3,
            "enabled": false,
            "proxy_selection": {
                "mode": "profile",
                "proxy_profile_id": ProxyProfileId::DIRECT
            }
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 3);
    assert_eq!(updated["items"][0]["label"], "Renamed OAuth");
    assert_eq!(updated["items"][0]["requests_per_minute"], 3);
    assert_eq!(updated["items"][0]["enabled"], false);
    assert_eq!(updated["items"][0]["proxy_selection"]["mode"], "profile");
    assert_eq!(
        updated["items"][0]["proxy_selection"]["proxy_profile_id"],
        ProxyProfileId::DIRECT.to_string()
    );
    assert_eq!(updated["items"][0]["config_version"], 2);
    assert_eq!(updated["items"][0]["account_generation"], 2);

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
            "requests_per_minute": 1,
            "enabled": true,
            "proxy_selection": {
                "mode": "profile",
                "proxy_profile_id": ProxyProfileId::DIRECT
            }
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

#[tokio::test]
async fn oauth_account_admin_lists_oldest_accounts_first() {
    let fixture = TestApplication::new().await;
    let publisher = fixture.publisher();
    let newer_id = OAuthAccountId::new();
    let older_id = OAuthAccountId::new();
    for (id, label, access_token, email) in [
        (newer_id, "A New", "newer-access", "newer@example.com"),
        (older_id, "Z Old", "older-access", "older@example.com"),
    ] {
        publisher
            .activate_oauth_account(
                id,
                ProviderKind::Codex,
                OAuthAccountDraft::new(label, None, true).expect("OAuth account draft"),
                OAuthProxySelection::Global,
                Some(email.to_owned()),
                Some(1_800_000_000),
                vec!["gpt-5.5".to_owned()],
                oauth_document(access_token, email),
            )
            .await
            .expect("activate OAuth account");
    }
    let (directory, old_router, storage) = fixture.into_router();
    drop(old_router);
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(directory.path().join("any2api.sqlite3")),
    )
    .await
    .expect("secondary SQLite connection");
    for (id, created_at) in [
        (newer_id, "2026-08-05 00:00:02"),
        (older_id, "2026-08-05 00:00:01"),
    ] {
        sqlx::query("UPDATE oauth_accounts SET created_at = ? WHERE id = ?")
            .bind(created_at)
            .bind(id.to_string())
            .execute(&mut connection)
            .await
            .expect("set account creation time");
    }
    drop(connection);

    let fixture = TestApplication::from_storage(directory, storage).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (status, listed) = request_json(
        fixture.router(),
        Method::GET,
        "/api/admin/oauth/accounts",
        None,
        loopback,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["config_revision"], 3);
    let items = listed["items"].as_array().expect("OAuth account items");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], older_id.to_string());
    assert_eq!(items[1]["id"], newer_id.to_string());
}

async fn test_app() -> (tempfile::TempDir, Router, Arc<SqliteStore>, OAuthAccountId) {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let runtime = fixture.runtime();
    let snapshots = fixture.snapshots();
    let publisher = fixture.publisher();
    let account_id = OAuthAccountId::new();
    publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            OAuthAccountDraft::new("Primary Codex OAuth", None, true)
            .expect("OAuth account draft"),
            OAuthProxySelection::Global,
            Some("person@example.com".to_owned()),
            Some(1_800_000_000),
            vec!["gpt-5.5".to_owned()],
            OAuthAccountDocument::new(
                ProviderKind::Codex,
                br#"{"access_token":"access-secret","refresh_token":"refresh-secret","id_token":null,"account_id":null,"email":"person@example.com"}"#
                    .to_vec()
                    .into(),
            )
            .expect("OAuth account document"),
        )
        .await
        .expect("activate OAuth account");
    storage
        .upsert_oauth_model_catalog_snapshot(&StoredOAuthModelCatalogSnapshot {
            provider_kind: ProviderKind::Codex,
            directory_scope: "free".to_owned(),
            fetched_at: 1_800_000_000,
            models: vec![
                "codex-auto-review".to_owned(),
                "gpt-5.4-mini".to_owned(),
                "gpt-5.5".to_owned(),
                "gpt-5.6-luna".to_owned(),
                "gpt-5.6-terra".to_owned(),
            ],
        })
        .await
        .expect("persist OAuth model catalog snapshot");
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        snapshots.load().revision(),
        snapshots.load().settings().logging(),
        &runtime.lifecycle(),
    ));
    let oauth = Arc::new(OAuthService::new(
        fixture.components().provider_registry_handle(),
        fixture.components().transport_manager(),
        publisher,
        Arc::clone(&storage),
        Arc::clone(&telemetry),
    ));
    let state = fixture
        .state()
        .with_oauth(oauth)
        .with_request_telemetry(telemetry);
    let (directory, app, _fixture_storage) = fixture.into_router_with_state(state);
    (directory, app, storage, account_id)
}

fn oauth_document(access_token: &str, email: &str) -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        serde_json::to_vec(&json!({
            "access_token": access_token,
            "refresh_token": null,
            "id_token": null,
            "account_id": null,
            "email": email,
        }))
        .expect("OAuth document JSON")
        .into(),
    )
    .expect("OAuth account document")
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
            client_ip: "127.0.0.1".parse().expect("loopback address"),
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("gpt-5.5".into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: Some(account_id),
            proxy_profile_id: Some(ProxyProfileId::DIRECT),
            status_code,
            error_class: None,
            error_message: None,
            attempt_count: 0,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            quota_cost: None,
            is_stream: false,
        },
        attempts: Vec::new(),
        telemetry_position: None,
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
