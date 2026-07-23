use std::{collections::HashMap, fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{
    ConfigPublisher, ProviderOAuthService, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{TransportManager, TransportRequest, TransportResponse};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CONTENT_TYPE},
};
use bytes::Bytes;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tower::ServiceExt;

#[tokio::test]
async fn provider_oauth_login_persists_only_a_redacted_credential() {
    let (_directory, app, _storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let endpoint = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Codex OAuth",
            "provider_kind": "codex",
            "base_url": "https://chatgpt.com/backend-api/codex",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    let endpoint_id = endpoint.body["items"][0]["id"]
        .as_str()
        .expect("endpoint id");

    let forged = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": 2,
            "label": "Forged OAuth",
            "credential_kind": "oauth2",
            "api_key": "not-an-oauth-token",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 1,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(forged.status, StatusCode::BAD_REQUEST);
    assert_eq!(forged.body["error"]["code"], "invalid_provider_credential");

    let started = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/oauth/start"),
        Some(json!({
            "expected_revision": 2,
            "label": "Personal OAuth",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 3,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(started.status, StatusCode::OK);
    assert_eq!(started.cache_control.as_deref(), Some("no-store"));
    let session_id = started.body["session_id"].as_str().expect("session id");
    let authorization_url = url::Url::parse(
        started.body["authorization_url"]
            .as_str()
            .expect("authorization URL"),
    )
    .expect("authorization URL parses");
    let state = authorization_url
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("OAuth state");
    assert_eq!(
        started.body["redirect_uri"],
        "http://localhost:1455/auth/callback"
    );

    let access_token = "oauth-access-token-must-never-be-returned";
    let refresh_token = "oauth-refresh-token-must-never-be-returned";
    let exchanged = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/oauth/exchange"),
        Some(json!({
            "session_id": session_id,
            "callback_url": format!(
                "http://localhost:1455/auth/callback?code=one-time-code&state={state}"
            )
        })),
        loopback,
    )
    .await;
    assert_eq!(exchanged.status, StatusCode::OK);
    assert_eq!(exchanged.cache_control.as_deref(), Some("no-store"));
    assert!(!exchanged.raw_body.contains(access_token));
    assert!(!exchanged.raw_body.contains(refresh_token));
    assert!(!exchanged.raw_body.contains("one-time-code"));
    let credential_id = exchanged.body["credential_id"]
        .as_str()
        .expect("credential id");

    let listed = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        None,
        loopback,
    )
    .await;
    assert_eq!(listed.body["items"][0]["id"], credential_id);
    assert_eq!(listed.body["items"][0]["credential_kind"], "oauth2");
    assert_eq!(listed.body["items"][0]["secret_tail"], Value::Null);
    assert!(!listed.raw_body.contains(access_token));
    assert!(!listed.raw_body.contains(refresh_token));

    let rotate = request_json(
        app,
        Method::POST,
        &format!("/api/admin/provider-credentials/{credential_id}/rotate-secret"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 1,
            "expected_secret_version": 1,
            "api_key": "must-not-replace-oauth"
        })),
        loopback,
    )
    .await;
    assert_eq!(rotate.status, StatusCode::BAD_REQUEST);
    assert_eq!(rotate.body["error"]["code"], "invalid_provider_credential");
}

#[tokio::test]
async fn provider_credential_crud_and_rotation_never_return_the_api_key() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let endpoint = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Codex Primary",
            "provider_kind": "codex",
            "base_url": "https://api.example.com",
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(endpoint.status, StatusCode::OK);
    let endpoint_id = endpoint.body["items"][0]["id"]
        .as_str()
        .expect("endpoint id")
        .to_owned();
    let create_key = "sk-contract-create-secret";

    let created = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": 2,
            "label": "Primary Key",
            "credential_kind": "api_key",
            "api_key": create_key,
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 4,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(created.status, StatusCode::OK);
    assert_eq!(created.cache_control.as_deref(), Some("no-store"));
    assert!(!created.raw_body.contains(create_key));
    assert_eq!(created.body["config_revision"], 3);
    assert_eq!(created.body["items"][0]["credential_kind"], "api_key");
    assert_eq!(created.body["items"][0]["secret_version"], 1);
    assert_eq!(created.body["items"][0]["config_version"], 1);
    let credential_id = created.body["items"][0]["id"]
        .as_str()
        .expect("credential id")
        .to_owned();

    let listed = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        None,
        loopback,
    )
    .await;
    assert_eq!(listed.status, StatusCode::OK);
    assert_eq!(listed.cache_control.as_deref(), Some("no-store"));
    assert!(!listed.raw_body.contains(create_key));

    let updated = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/provider-credentials/{credential_id}"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 1,
            "label": "Primary Key Updated",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 8,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["items"][0]["config_version"], 2);
    assert_eq!(updated.body["items"][0]["secret_version"], 1);

    let rotate_key = "sk-contract-rotated-secret";
    let rotated = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-credentials/{credential_id}/rotate-secret"),
        Some(json!({
            "expected_revision": 4,
            "expected_config_version": 2,
            "expected_secret_version": 1,
            "api_key": rotate_key
        })),
        loopback,
    )
    .await;
    assert_eq!(rotated.status, StatusCode::OK);
    assert!(!rotated.raw_body.contains(rotate_key));
    assert_eq!(rotated.body["items"][0]["config_version"], 3);
    assert_eq!(rotated.body["items"][0]["secret_version"], 2);
    assert_eq!(rotated.body["items"][0]["credential_generation"], 2);

    let stale = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-credentials/{credential_id}/rotate-secret"),
        Some(json!({
            "expected_revision": 5,
            "expected_config_version": 3,
            "expected_secret_version": 1,
            "api_key": "sk-stale-rotation"
        })),
        loopback,
    )
    .await;
    assert_eq!(stale.status, StatusCode::CONFLICT);
    assert_eq!(
        stale.body["error"]["code"],
        "provider_credential_secret_version_conflict"
    );

    let endpoint_in_use = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/provider-endpoints/{endpoint_id}?expected_revision=5"),
        None,
        loopback,
    )
    .await;
    assert_eq!(endpoint_in_use.status, StatusCode::CONFLICT);
    assert_eq!(
        endpoint_in_use.body["error"]["code"],
        "provider_endpoint_in_use"
    );

    let deleted = request_json(
        app,
        Method::DELETE,
        &format!(
            "/api/admin/provider-credentials/{credential_id}?expected_revision=5&expected_config_version=3"
        ),
        None,
        loopback,
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK);
    assert_eq!(deleted.body["config_revision"], 6);
    assert_eq!(deleted.body["items"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        storage
            .load_configuration()
            .await
            .expect("configuration")
            .provider_credentials()
            .credentials()
            .len(),
        0
    );
}

#[tokio::test]
async fn provider_credential_requests_reject_unknown_secret_fields() {
    let (_directory, app, _storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let response = request_json(
        app,
        Method::PATCH,
        "/api/admin/provider-credentials/00000000-0000-0000-0000-000000000001",
        Some(json!({
            "expected_revision": 1,
            "expected_config_version": 1,
            "label": "Unexpected Secret",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 1,
            "enabled": true,
            "api_key": "must-not-be-accepted"
        })),
        loopback,
    )
    .await;

    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.body["error"]["code"], "invalid_request");
    assert_eq!(response.cache_control.as_deref(), Some("no-store"));
}

#[tokio::test]
async fn successful_credential_test_clears_generation_auth_error() {
    let (upstream_address, mut upstream_requests) = credential_test_upstream().await;
    let (_directory, app, _storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let endpoint = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/provider-endpoints",
        Some(json!({
            "expected_revision": 1,
            "name": "Local Codex",
            "provider_kind": "codex",
            "base_url": format!("http://{upstream_address}/v1"),
            "protocol_dialect": "openai_responses",
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(endpoint.status, StatusCode::OK);
    let endpoint_id = endpoint.body["items"][0]["id"]
        .as_str()
        .expect("endpoint id")
        .to_owned();

    let credential = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-endpoints/{endpoint_id}/credentials"),
        Some(json!({
            "expected_revision": 2,
            "label": "Recoverable Key",
            "credential_kind": "api_key",
            "api_key": "sk-credential-test-secret",
            "proxy_profile_id": "00000000-0000-0000-0000-000000000000",
            "max_concurrency": 1,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(credential.status, StatusCode::OK);
    let credential_id = credential.body["items"][0]["id"]
        .as_str()
        .expect("credential id")
        .to_owned();

    let models = request_json(
        app.clone(),
        Method::PUT,
        &format!("/api/admin/provider-credentials/{credential_id}/models"),
        Some(json!({
            "expected_revision": 3,
            "expected_config_version": 1,
            "models": ["upstream-model"]
        })),
        loopback,
    )
    .await;
    assert_eq!(models.status, StatusCode::OK);

    let gateway = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/gateway-api-keys",
        Some(json!({
            "expected_revision": 4,
            "name": "Credential test client",
            "enabled": true
        })),
        loopback,
    )
    .await;
    let gateway_token = gateway.body["token"].as_str().expect("gateway token");

    let failed = request_json_with_headers(
        app.clone(),
        Method::POST,
        "/v1/responses",
        Some(json!({"model": "upstream-model", "input": "hello"})),
        loopback,
        &[("authorization", format!("Bearer {gateway_token}"))],
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_GATEWAY);
    let first = upstream_requests
        .recv()
        .await
        .expect("first upstream request");
    assert_eq!(first.method, Method::POST);
    assert_eq!(first.path, "/v1/responses");
    assert_eq!(
        first.headers["authorization"],
        "Bearer sk-credential-test-secret"
    );

    let tested = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/provider-credentials/{credential_id}/test"),
        None,
        loopback,
    )
    .await;
    assert_eq!(tested.status, StatusCode::OK);
    assert_eq!(tested.body["config_revision"], 5);
    assert_eq!(tested.body["credential_id"], credential_id);
    assert_eq!(tested.body["reachable"], true);
    assert_eq!(tested.body["accepted"], true);
    assert_eq!(tested.body["catalog_valid"], true);
    assert_eq!(tested.body["models"], json!([]));
    assert_eq!(tested.body["status_code"], 200);
    assert_eq!(tested.body["auth_error_cleared"], true);
    assert!(!tested.raw_body.contains("sk-credential-test-secret"));
    let probe = upstream_requests
        .recv()
        .await
        .expect("credential probe request");
    assert_eq!(probe.method, Method::GET);
    assert_eq!(probe.path, "/v1/models");
    assert_eq!(
        probe.headers["authorization"],
        "Bearer sk-credential-test-secret"
    );

    let recovered = request_json_with_headers(
        app,
        Method::POST,
        "/v1/responses",
        Some(json!({"model": "upstream-model", "input": "hello again"})),
        loopback,
        &[("authorization", format!("Bearer {gateway_token}"))],
    )
    .await;
    assert_eq!(recovered.status, StatusCode::OK);
    let final_request = upstream_requests
        .recv()
        .await
        .expect("recovered upstream request");
    assert_eq!(final_request.method, Method::POST);
    assert_eq!(final_request.path, "/v1/responses");
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
    let components = build_public_request_components().expect("public request components");
    let public_requests = components.service();
    let provider_oauth = Arc::new(ProviderOAuthService::new(
        components.provider_registry_handle(),
        Arc::new(FakeOAuthTransport),
        Arc::clone(&snapshots),
        Arc::clone(&publisher),
    ));
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, public_requests)
            .with_provider_credential_tests(components.provider_credential_test_service())
            .with_provider_oauth(provider_oauth),
        web_root,
    );
    (directory, app, storage)
}

struct FakeOAuthTransport;

#[async_trait]
impl TransportManager for FakeOAuthTransport {
    async fn execute(
        &self,
        _proxy: any2api_transport::api::TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(request.method, Method::POST);
        let body = Bytes::from_static(
            br#"{"access_token":"oauth-access-token-must-never-be-returned","refresh_token":"oauth-refresh-token-must-never-be-returned","expires_in":3600}"#,
        );
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: axum::http::HeaderMap::new(),
            body: Box::pin(futures_util::stream::iter([Ok(body)])),
            read_failure_scope: any2api_transport::api::TransportFailureScope::Endpoint,
        })
    }
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
    request_json_with_headers(app, method, uri, body, remote, &[]).await
}

async fn request_json_with_headers(
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

struct UpstreamRequest {
    method: Method,
    path: String,
    headers: HashMap<String, String>,
}

async fn credential_test_upstream() -> (SocketAddr, mpsc::UnboundedReceiver<UpstreamRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("upstream listener");
    let address = listener.local_addr().expect("upstream address");
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let responses = [
            (
                "401 Unauthorized",
                r#"{"error":{"type":"authentication_error","code":"invalid_api_key"}}"#,
            ),
            ("200 OK", r#"{"object":"list","data":[]}"#),
            (
                "200 OK",
                r#"{"id":"resp_recovered","model":"upstream-model"}"#,
            ),
        ];
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().await.expect("upstream accept");
            sender.send(read_upstream_request(&mut stream).await).ok();
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("upstream response");
        }
    });
    (address, receiver)
}

async fn read_upstream_request(stream: &mut TcpStream) -> UpstreamRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).await.expect("upstream read");
        assert!(count > 0, "request ended before headers");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position;
        }
    };
    let head = String::from_utf8(bytes[..header_end].to_vec()).expect("request headers");
    let content_length = head
        .lines()
        .find_map(|line| {
            line.split_once(':').and_then(|(name, value)| {
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
        })
        .unwrap_or(0);
    let body_end = header_end + 4 + content_length;
    while bytes.len() < body_end {
        let count = stream.read(&mut buffer).await.expect("upstream body read");
        assert!(count > 0, "request ended before body");
        bytes.extend_from_slice(&buffer[..count]);
    }
    let mut lines = head.lines();
    let mut request_line = lines.next().expect("request line").split_whitespace();
    let method = request_line
        .next()
        .expect("request method")
        .parse()
        .expect("valid request method");
    let path = request_line.next().expect("request path").to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    UpstreamRequest {
        method,
        path,
        headers,
    }
}
