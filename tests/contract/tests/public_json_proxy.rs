use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components_with_telemetry;
use any2api_domain::{MAX_TOKEN_COUNT, RequestId};
use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{HeaderMap, Method, Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{mpsc, oneshot},
};
use tower::ServiceExt;

#[tokio::test]
async fn codex_responses_uses_upstream_path_and_provider_key() {
    let upstream_body = json!({
        "id": "resp_1",
        "model": "gpt-upstream",
        "output": [],
        "usage": {
            "input_tokens": 0,
            "output_tokens": MAX_TOKEN_COUNT,
            "input_tokens_details": {"cache_write_tokens": 0}
        }
    })
    .to_string();
    let (listener, upstream) = upstream_server_with_headers(
        "/v1/responses",
        &upstream_body,
        &[
            ("Authorization", "Bearer provider-secret"),
            ("X-Api-Key", "provider-secret"),
            ("Set-Cookie", "provider-secret=1"),
            ("Connection", "x-private-hop"),
            ("X-Private-Hop", "provider-secret"),
            ("ETag", "\"upstream-body\""),
            ("X-Request-Id", "upstream-request-1"),
        ],
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Codex local",
        "codex",
        &format!("http://{listener}/v1"),
    )
    .await;
    let credential_id = create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-codex-contract",
    )
    .await;
    select_models(&app, loopback, revision + 3, &endpoint_id, "gpt-upstream").await;

    let response = request_json(
        app.clone(),
        Method::POST,
        "/v1/responses",
        Some(json!({
            "model": "gpt-upstream",
            "input": "hello",
            "stream": false,
            "unknown_field": {"keep": true}
        })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["model"], "gpt-upstream");
    assert_eq!(response.body["id"], "resp_1");
    assert!(response.headers.get("authorization").is_none());
    assert!(response.headers.get("x-api-key").is_none());
    assert!(response.headers.get("set-cookie").is_none());
    assert!(response.headers.get("x-private-hop").is_none());
    assert!(response.headers.get("etag").is_none());
    let request_id = response.headers["x-request-id"]
        .to_str()
        .expect("request ID header")
        .parse::<RequestId>()
        .expect("local request ID");
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.method, Method::POST);
    assert_eq!(request.path, "/v1/responses");
    assert_eq!(
        request.headers.get("authorization"),
        Some(&"Bearer sk-codex-contract".to_owned())
    );
    assert!(!request.headers.contains_key("x-api-key"));
    assert_eq!(request.body["model"], "gpt-upstream");
    assert_eq!(request.body["unknown_field"]["keep"], true);
    let logs = wait_for_request_log(&app, loopback, request_id).await;
    assert_eq!(logs["request"]["request_id"], request_id.to_string());
    assert_eq!(logs["request"]["credential_id"], credential_id);
    assert_eq!(logs["request"]["attempt_count"], 1);
    assert_eq!(logs["request"]["first_token_ms"], Value::Null);
    assert_eq!(logs["request"]["input_tokens"], 0);
    assert_eq!(logs["request"]["output_tokens"], json!(MAX_TOKEN_COUNT));
    assert_eq!(logs["request"]["cache_read_tokens"], Value::Null);
    assert_eq!(logs["request"]["cache_write_tokens"], 0);
    assert_eq!(logs["attempts"][0]["outcome"], "success");
    assert_eq!(logs["attempts"][0]["status_code"], 200);

    let list = request_json(
        app,
        Method::GET,
        "/api/admin/request-logs?limit=10",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.body["items"][0]["request_id"], request_id.to_string());
    assert_eq!(list.body["items"][0]["first_token_ms"], Value::Null);
    assert_eq!(list.body["items"][0]["input_tokens"], 0);
    assert_eq!(
        list.body["items"][0]["output_tokens"],
        json!(MAX_TOKEN_COUNT)
    );
    assert_eq!(list.body["items"][0]["cache_read_tokens"], Value::Null);
    assert_eq!(list.body["items"][0]["cache_write_tokens"], 0);
}

#[tokio::test]
async fn responses_compact_uses_its_distinct_non_streaming_path() {
    let (listener, upstream) = upstream_server(
        "/v1/responses/compact",
        r#"{"id":"cmp_1","model":"gpt-upstream","output":[]}"#,
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Codex compact",
        "codex",
        &format!("http://{listener}/v1"),
    )
    .await;
    create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-compact-contract",
    )
    .await;
    select_models(&app, loopback, revision + 3, &endpoint_id, "gpt-upstream").await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/responses/compact",
        Some(json!({
            "model": "gpt-upstream",
            "input": [{"role":"user","content":"compact this"}]
        })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["model"], "gpt-upstream");
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.path, "/v1/responses/compact");
    assert_eq!(request.body["model"], "gpt-upstream");
}

#[tokio::test]
async fn claude_messages_uses_anthropic_headers_and_path() {
    let (listener, upstream) = upstream_server(
        "/v1/messages",
        r#"{"id":"msg_1","type":"message","model":"claude-upstream","content":[]}"#,
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Claude local",
        "claude",
        &format!("http://{listener}/v1"),
    )
    .await;
    create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-claude-contract",
    )
    .await;
    select_models(
        &app,
        loopback,
        revision + 3,
        &endpoint_id,
        "claude-upstream",
    )
    .await;

    let response = request_json(
        app,
        Method::POST,
        "/v1/messages",
        Some(json!({
            "model": "claude-upstream",
            "max_tokens": 32,
            "messages": [{"role":"user","content":"hello"}]
        })),
        loopback,
        &[
            ("x-api-key", token),
            ("anthropic-beta", "messages-2024-09-04".to_owned()),
        ],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body["model"], "claude-upstream");
    let request = upstream.await.expect("upstream request");
    assert_eq!(request.path, "/v1/messages");
    assert_eq!(
        request.headers.get("x-api-key"),
        Some(&"sk-claude-contract".to_owned())
    );
    assert_eq!(
        request.headers.get("anthropic-version"),
        Some(&"2023-06-01".to_owned())
    );
    assert_eq!(
        request.headers.get("anthropic-beta"),
        Some(&"messages-2024-09-04".to_owned())
    );
    assert!(!request.headers.contains_key("authorization"));
    assert_eq!(request.body["model"], "claude-upstream");
}

#[tokio::test]
async fn affinity_admin_exposes_redacted_runtime_state_and_clears_by_credential() {
    let (listener, upstream) = upstream_server(
        "/v1/responses",
        r#"{"id":"resp_affinity","model":"gpt-upstream","output":[]}"#,
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Codex affinity",
        "codex",
        &format!("http://{listener}/v1"),
    )
    .await;
    let credential_id = create_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "sk-affinity-contract",
    )
    .await;
    select_models(&app, loopback, revision + 3, &endpoint_id, "gpt-upstream").await;

    let response = request_json(
        app.clone(),
        Method::POST,
        "/v1/responses",
        Some(json!({
            "model": "gpt-upstream",
            "input": "bind this session"
        })),
        loopback,
        &[
            ("authorization", format!("Bearer {token}")),
            ("x-any2api-session", "private-session-id".to_owned()),
        ],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    upstream.await.expect("upstream request");

    let affinity = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/affinity?limit=10",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(affinity.status, StatusCode::OK);
    assert_eq!(affinity.headers["cache-control"], "no-store");
    assert_eq!(affinity.body["soft_binding_count"], 1);
    assert_eq!(affinity.body["hard_binding_count"], 1);
    assert_eq!(affinity.body["creating_count"], 0);
    assert_eq!(
        affinity.body["credential_counts"][0]["credential_id"],
        credential_id
    );
    assert_eq!(
        affinity.body["credential_counts"][0]["credential_label"],
        "primary"
    );
    for binding in affinity.body["bindings"]
        .as_array()
        .expect("binding samples")
    {
        let hash = binding["session_hash_prefix"]
            .as_str()
            .expect("redacted hash");
        assert_eq!(hash.len(), 12);
        assert!(!hash.contains("private-session-id"));
        assert!(!hash.contains("resp_affinity"));
    }

    let cleared = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/affinity/credentials/{credential_id}"),
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(cleared.status, StatusCode::OK);
    assert_eq!(cleared.body["cleared_count"], 2);

    let empty = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/affinity",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(empty.body["soft_binding_count"], 0);
    assert_eq!(empty.body["hard_binding_count"], 0);

    let cleared_all = request_json(
        app,
        Method::DELETE,
        "/api/admin/affinity",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(cleared_all.status, StatusCode::OK);
    assert_eq!(cleared_all.body["cleared_count"], 0);
}

#[tokio::test]
async fn previous_response_id_stays_on_the_original_credential() {
    let (listener, mut upstream) = upstream_server_sequence(
        "/v1/responses",
        &[
            r#"{"id":"resp_sticky","model":"gpt-upstream","output":[]}"#,
            r#"{"id":"resp_follow","model":"gpt-upstream","output":[]}"#,
        ],
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Codex hard affinity",
        "codex",
        &format!("http://{listener}/v1"),
    )
    .await;
    create_labeled_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "first",
        "sk-hard-first",
    )
    .await;
    create_labeled_credential(
        &app,
        loopback,
        revision + 3,
        &endpoint_id,
        "second",
        "sk-hard-second",
    )
    .await;
    select_models(&app, loopback, revision + 4, &endpoint_id, "gpt-upstream").await;

    let first_response = request_json(
        app.clone(),
        Method::POST,
        "/v1/responses",
        Some(json!({ "model": "gpt-upstream", "input": "start" })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(first_response.status, StatusCode::OK);
    let first = upstream.recv().await.expect("first upstream request");

    let second_response = request_json(
        app.clone(),
        Method::POST,
        "/v1/responses",
        Some(json!({
            "model": "gpt-upstream",
            "previous_response_id": "resp_sticky",
            "input": "continue"
        })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(second_response.status, StatusCode::OK);
    let second = upstream.recv().await.expect("second upstream request");
    assert_eq!(
        first.headers.get("authorization"),
        second.headers.get("authorization")
    );

    let lost = request_json(
        app,
        Method::POST,
        "/v1/responses",
        Some(json!({
            "model": "gpt-upstream",
            "previous_response_id": "resp_from_an_old_process",
            "input": "continue"
        })),
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(lost.status, StatusCode::CONFLICT);
    assert_eq!(lost.body["error"]["code"], "session_binding_lost");
}

#[tokio::test]
async fn claude_explicit_soft_session_stays_on_the_original_credential() {
    let (listener, mut upstream) = upstream_server_sequence(
        "/v1/messages",
        &[
            r#"{"id":"msg_soft_1","type":"message","model":"claude-upstream","content":[]}"#,
            r#"{"id":"msg_soft_2","type":"message","model":"claude-upstream","content":[]}"#,
        ],
    )
    .await;
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let token = create_gateway_key(&app, loopback, revision).await;
    let endpoint_id = create_endpoint(
        &app,
        loopback,
        revision + 1,
        "Claude soft affinity",
        "claude",
        &format!("http://{listener}/v1"),
    )
    .await;
    create_labeled_credential(
        &app,
        loopback,
        revision + 2,
        &endpoint_id,
        "first",
        "sk-soft-first",
    )
    .await;
    create_labeled_credential(
        &app,
        loopback,
        revision + 3,
        &endpoint_id,
        "second",
        "sk-soft-second",
    )
    .await;
    select_models(
        &app,
        loopback,
        revision + 4,
        &endpoint_id,
        "claude-upstream",
    )
    .await;

    for input in ["start", "continue"] {
        let response = request_json(
            app.clone(),
            Method::POST,
            "/v1/messages",
            Some(json!({
                "model": "claude-upstream",
                "max_tokens": 16,
                "messages": [{"role":"user","content":input}]
            })),
            loopback,
            &[
                ("x-api-key", token.clone()),
                ("x-any2api-session", "claude-session-one".to_owned()),
            ],
        )
        .await;
        assert_eq!(response.status, StatusCode::OK);
    }
    let first = upstream.recv().await.expect("first upstream request");
    let second = upstream.recv().await.expect("second upstream request");
    assert_eq!(
        first.headers.get("x-api-key"),
        second.headers.get("x-api-key")
    );
}

#[tokio::test]
async fn public_ingress_errors_require_authentication_and_use_protocol_envelopes() {
    let (_directory, app, revision) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let missing = request_json(
        app.clone(),
        Method::POST,
        "/v1/messages/not-a-route",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(missing.body["type"], "error");
    assert_eq!(missing.body["error"]["type"], "authentication_error");
    assert_eq!(missing.headers["cache-control"], "no-store");
    assert!(
        missing.headers["x-request-id"]
            .to_str()
            .expect("request ID")
            .parse::<RequestId>()
            .is_ok()
    );

    let openai_missing = request_json(
        app.clone(),
        Method::POST,
        "/v1/responses/compact",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(openai_missing.status, StatusCode::UNAUTHORIZED);
    assert_eq!(openai_missing.body["error"]["type"], "authentication_error");
    assert_eq!(openai_missing.body["error"]["code"], "unauthorized");
    assert_eq!(openai_missing.headers["cache-control"], "no-store");

    let token = create_gateway_key(&app, loopback, revision).await;
    let conflicting = request_json(
        app.clone(),
        Method::POST,
        "/v1/messages",
        None,
        loopback,
        &[
            ("authorization", format!("Bearer {token}")),
            ("x-api-key", "different".to_owned()),
        ],
    )
    .await;
    assert_eq!(conflicting.status, StatusCode::BAD_REQUEST);
    assert_eq!(conflicting.body["type"], "error");
    assert_eq!(conflicting.body["error"]["type"], "invalid_request_error");

    let unknown = request_json(
        app.clone(),
        Method::GET,
        "/v1/not-a-route",
        None,
        loopback,
        &[("x-api-key", token.clone())],
    )
    .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert_eq!(unknown.body["error"]["type"], "invalid_request_error");
    assert_eq!(unknown.body["error"]["code"], "public_api_not_found");
    assert_eq!(unknown.headers["cache-control"], "no-store");
    assert!(
        unknown.headers["x-request-id"]
            .to_str()
            .expect("request ID")
            .parse::<RequestId>()
            .is_ok()
    );

    let wrong_method = request_json(
        app.clone(),
        Method::GET,
        "/v1/responses",
        None,
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(wrong_method.body["error"]["type"], "invalid_request_error");
    assert_eq!(wrong_method.body["error"]["code"], "method_not_allowed");
    assert!(
        wrong_method.headers["x-request-id"]
            .to_str()
            .expect("request ID")
            .parse::<RequestId>()
            .is_ok()
    );

    let claude_wrong_method = request_json(
        app.clone(),
        Method::GET,
        "/v1/messages/count_tokens",
        None,
        loopback,
        &[("x-api-key", token.clone())],
    )
    .await;
    assert_eq!(claude_wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(claude_wrong_method.body["type"], "error");
    assert_eq!(
        claude_wrong_method.body["error"]["type"],
        "invalid_request_error"
    );

    let claude_unknown = request_json(
        app,
        Method::GET,
        "/v1/messages/not-a-route",
        None,
        loopback,
        &[("authorization", format!("Bearer {token}"))],
    )
    .await;
    assert_eq!(claude_unknown.status, StatusCode::NOT_FOUND);
    assert_eq!(claude_unknown.body["type"], "error");
    assert_eq!(claude_unknown.body["error"]["type"], "not_found_error");
}

async fn test_app() -> (tempfile::TempDir, Router, u64) {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
        &runtime.lifecycle(),
    ));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let publisher = Arc::new(ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    ));
    let service = build_public_request_components_with_telemetry(Arc::clone(&telemetry))
        .expect("public request components")
        .service();
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let revision = snapshots.load().revision().get();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, service).with_request_telemetry(telemetry),
        web_root,
    );
    (directory, app, revision)
}

async fn wait_for_request_log(app: &Router, remote: SocketAddr, request_id: RequestId) -> Value {
    for _ in 0..200 {
        let response = request_json(
            app.clone(),
            Method::GET,
            &format!("/api/admin/request-logs/{request_id}"),
            None,
            remote,
            &[],
        )
        .await;
        if response.status == StatusCode::OK {
            return response.body;
        }
        assert_eq!(response.status, StatusCode::NOT_FOUND);
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("request log was not persisted");
}

async fn create_gateway_key(app: &Router, remote: SocketAddr, revision: u64) -> String {
    let response = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({"expected_revision": revision, "name":"client", "enabled":true})),
        remote,
        &[],
    )
    .await;
    response.body["token"].as_str().expect("token").to_owned()
}

async fn create_endpoint(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    name: &str,
    provider_kind: &str,
    base_url: &str,
) -> String {
    let dialect = if provider_kind == "codex" {
        "openai_responses"
    } else {
        "anthropic_messages"
    };
    let response = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": revision,
            "name": name,
            "provider_kind": provider_kind,
            "base_url": base_url,
            "protocol_dialect": dialect,
            "enabled": true
        })),
        remote,
        &[],
    )
    .await;
    response.body["items"][0]["id"]
        .as_str()
        .expect("endpoint")
        .to_owned()
}

async fn create_credential(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    api_key: &str,
) -> String {
    create_labeled_credential(app, remote, revision, endpoint_id, "primary", api_key).await
}

async fn create_labeled_credential(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    label: &str,
    api_key: &str,
) -> String {
    let response = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": revision,
            "label": label,
            "credential_kind": "api_key",
            "api_key": api_key,
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 2,
            "enabled": true
        })),
        remote,
        &[],
    )
    .await;
    response.body["items"][0]["id"]
        .as_str()
        .expect("credential")
        .to_owned()
}

async fn select_models(
    app: &Router,
    remote: SocketAddr,
    revision: u64,
    endpoint_id: &str,
    model: &str,
) {
    let listed = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        None,
        remote,
        &[],
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    let credentials = listed.body["items"]
        .as_array()
        .expect("credential items")
        .iter()
        .map(|credential| {
            (
                credential["id"].as_str().expect("credential id").to_owned(),
                credential["config_version"]
                    .as_u64()
                    .expect("credential config version"),
            )
        })
        .collect::<Vec<_>>();
    assert!(!credentials.is_empty());

    for (offset, (credential_id, config_version)) in credentials.into_iter().enumerate() {
        let response = request_json(
            app.clone(),
            Method::PUT,
            &format!("/api/admin/provider-credentials/{credential_id}/models"),
            Some(json!({
                "expected_revision": revision + offset as u64,
                "expected_config_version": config_version,
                "models": [model]
            })),
            remote,
            &[],
        )
        .await;
        assert_eq!(response.status, StatusCode::OK);
    }
}

struct JsonResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
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
        Body::from(serde_json::to_vec(&value).expect("request JSON"))
    } else {
        Body::empty()
    };
    let response = app
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    JsonResponse {
        status,
        headers,
        body: serde_json::from_slice(&bytes).expect("JSON response"),
    }
}

#[derive(Debug)]
struct UpstreamRequest {
    method: Method,
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: Value,
}

async fn upstream_server(
    expected_path: &str,
    response_body: &str,
) -> (SocketAddr, oneshot::Receiver<UpstreamRequest>) {
    upstream_server_with_headers(expected_path, response_body, &[]).await
}

async fn upstream_server_with_headers(
    expected_path: &str,
    response_body: &str,
    response_headers: &[(&str, &str)],
) -> (SocketAddr, oneshot::Receiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = oneshot::channel();
    let expected_path = expected_path.to_owned();
    let response_body = response_body.to_owned();
    let response_headers = response_headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("upstream accept");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = stream.read(&mut buffer).await.expect("upstream read");
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let header_end = bytes
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("request boundary");
        let head = String::from_utf8(bytes[..header_end].to_vec()).expect("upstream headers");
        let content_length = head
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().expect("content length"))
                })
            })
            .unwrap_or(0);
        let body_start = header_end + 4;
        while bytes.len() < body_start + content_length {
            let count = stream.read(&mut buffer).await.expect("upstream body read");
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        let body = String::from_utf8(bytes[body_start..].to_vec()).expect("upstream body text");
        let mut lines = head.lines();
        let request_line = lines.next().expect("request line");
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .expect("request method")
            .parse::<Method>()
            .expect("valid request method");
        let path = request_parts.next().expect("request path");
        assert_eq!(path, expected_path);
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
            .collect::<std::collections::HashMap<_, _>>();
        let body = serde_json::from_str(body.trim()).expect("upstream JSON body");
        sender
            .send(UpstreamRequest {
                method,
                path: path.to_owned(),
                headers,
                body,
            })
            .expect("send upstream request");
        let extra_headers = response_headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n{}",
            response_body.len(),
            extra_headers,
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("upstream write");
    });
    (address, receiver)
}

async fn upstream_server_sequence(
    expected_path: &str,
    response_bodies: &[&str],
) -> (SocketAddr, mpsc::UnboundedReceiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = mpsc::unbounded_channel();
    let expected_path = expected_path.to_owned();
    let response_bodies = response_bodies
        .iter()
        .map(|body| (*body).to_owned())
        .collect::<Vec<_>>();
    tokio::spawn(async move {
        for response_body in response_bodies {
            let (mut stream, _) = listener.accept().await.expect("upstream accept");
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let count = stream.read(&mut buffer).await.expect("upstream read");
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            let header_end = bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("request boundary");
            let head = String::from_utf8(bytes[..header_end].to_vec()).expect("upstream headers");
            let content_length = head
                .lines()
                .find_map(|line| {
                    line.split_once(':').and_then(|(name, value)| {
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().expect("content length"))
                    })
                })
                .unwrap_or(0);
            let body_start = header_end + 4;
            while bytes.len() < body_start + content_length {
                let count = stream.read(&mut buffer).await.expect("upstream body read");
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            let body = String::from_utf8(bytes[body_start..].to_vec()).expect("upstream body text");
            let mut lines = head.lines();
            let request_line = lines.next().expect("request line");
            let mut request_parts = request_line.split_whitespace();
            let method = request_parts
                .next()
                .expect("request method")
                .parse::<Method>()
                .expect("valid request method");
            let path = request_parts.next().expect("request path");
            assert_eq!(path, expected_path);
            let headers = lines
                .filter_map(|line| line.split_once(':'))
                .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
                .collect::<std::collections::HashMap<_, _>>();
            let body = serde_json::from_str(body.trim()).expect("upstream JSON body");
            sender
                .send(UpstreamRequest {
                    method,
                    path: path.to_owned(),
                    headers,
                    body,
                })
                .expect("send upstream request");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("upstream write");
        }
    });
    (address, receiver)
}
