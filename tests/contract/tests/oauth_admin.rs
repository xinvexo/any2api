use std::{
    fs,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::ProxyProfileId;
use any2api_provider::{CodexDriver, ProviderRegistry};
use any2api_runtime::api::{
    ConfigPublisher, OAuthService, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    TransportFailureScope, TransportManager, TransportProxy, TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{
        HeaderMap, Method, Request, StatusCode,
        header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE},
    },
};
use http_body_util::BodyExt;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn oauth_start_is_loopback_protected_and_does_not_publish_configuration() {
    let (directory, app, storage) = test_app().await;
    let remote = SocketAddr::from(([203, 0, 113, 10], 41000));
    let (status, body) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "codex"})),
        remote,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");

    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (status, start) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "codex"})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(start["provider"], "codex");
    assert!(
        start["authorization_url"]
            .as_str()
            .is_some_and(|url| url.starts_with("https://auth.openai.com/"))
    );
    assert_eq!(start["redirect_uri"], "http://localhost:1455/auth/callback");
    assert_eq!(start["expires_in_seconds"], 600);

    let configuration = storage.load_configuration().await.expect("configuration");
    assert_eq!(configuration.revision().get(), 1);
    assert!(configuration.provider_endpoints().endpoints().is_empty());
    drop(directory);
}

#[tokio::test]
async fn oauth_exchange_rejects_unknown_sessions_without_network_access() {
    let (_directory, app, _storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (status, body) = request_json(
        app,
        Method::POST,
        "/api/admin/oauth/exchange",
        Some(json!({
            "session_id": "unknown",
            "callback_url": "http://localhost:1455/auth/callback?code=abc&state=state"
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "oauth_session_invalid");
}

#[tokio::test]
async fn oauth_exchange_activates_persisted_account_once_over_direct_transport() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (_, start) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "codex"})),
        loopback,
    )
    .await;
    let session_id = start["session_id"].as_str().expect("session id");
    let authorization_url = start["authorization_url"]
        .as_str()
        .expect("authorization URL");
    let state = url::Url::parse(authorization_url)
        .expect("authorization URL")
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("OAuth state");
    let callback_url =
        format!("http://localhost:1455/auth/callback?code=authorization-code&state={state}");

    let response = request(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/exchange",
        Some(json!({
            "session_id": session_id,
            "callback_url": callback_url
        })),
        loopback,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response
            .headers
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert!(response.headers.get(CONTENT_DISPOSITION).is_none());
    let activation: Value = serde_json::from_slice(&response.body).expect("activation response");
    assert_eq!(activation["provider"], "codex");
    assert_eq!(activation["max_concurrency"], 1);
    assert_eq!(activation["enabled"], true);
    assert_eq!(activation["selected_model_count"], 8);
    assert_eq!(activation["config_version"], 1);
    assert_eq!(activation["config_revision"], 2);
    // Default label prefers email when present; no provider prefix (UI already groups by kind).
    assert_eq!(activation["label"], "person@example.com");
    assert!(activation.get("access_token").is_none());
    assert!(activation.get("refresh_token").is_none());
    assert!(activation.get("id_token").is_none());
    assert_eq!(activation["safe_account_email"], "person@example.com");

    let stored = storage.load_configuration().await.expect("configuration");
    assert_eq!(stored.revision().get(), 2);
    let account = stored
        .oauth_accounts()
        .accounts()
        .first()
        .expect("persisted OAuth account");
    assert_eq!(activation["account_id"], account.id().to_string());
    assert_eq!(account.provider_kind(), any2api_domain::ProviderKind::Codex);
    assert_eq!(account.max_concurrency().get(), 1);
    assert!(account.enabled());
    assert_eq!(account.models().len(), 8);
    assert!(
        account
            .models()
            .iter()
            .any(|model| model.as_str() == "gpt-5.3-codex-spark")
    );
    let account_id = account.id();

    let mut materials = stored.into_parts().oauth_account_materials.into_entries();
    let material = materials.pop().expect("persisted OAuth document");
    assert!(materials.is_empty());
    assert_eq!(material.account_id(), account_id);
    let document = material.into_document().into_bytes();
    let document: Value =
        serde_json::from_slice(document.expose_secret()).expect("stored OAuth document");
    assert_eq!(document["type"], "codex");
    assert_eq!(document["access_token"], "access-token");
    assert_eq!(document["refresh_token"], "refresh-token");

    let (status, replay) = request_json(
        app,
        Method::POST,
        "/api/admin/oauth/exchange",
        Some(json!({
            "session_id": session_id,
            "callback_url": callback_url
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(replay["error"]["code"], "oauth_session_invalid");
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
    let components = build_public_request_components().expect("public request components");
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    let token_transport = Arc::new(TokenTransport::default());
    let oauth = Arc::new(OAuthService::new(
        Arc::new(providers),
        token_transport as Arc<dyn TransportManager>,
        Arc::clone(&publisher),
    ));
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, components.service()).with_oauth(oauth),
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
    let response = request(app, method, uri, body, remote).await;
    let value = serde_json::from_slice(&response.body).expect("response json");
    (response.status, value)
}

struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: bytes::Bytes,
}

async fn request(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
) -> TestResponse {
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
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    TestResponse {
        status,
        headers,
        body: bytes,
    }
}

#[derive(Default)]
struct TokenTransport {
    called: AtomicBool,
}

#[async_trait]
impl TransportManager for TokenTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        assert_eq!(request.uri.host(), Some("auth.openai.com"));
        assert!(!self.called.swap(true, Ordering::SeqCst));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async {
                Ok(bytes::Bytes::from_static(
                    br#"{"access_token":"access-token","refresh_token":"refresh-token","id_token":"header.eyJlbWFpbCI6InBlcnNvbkBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NvdW50LTEyMyIsImNoYXRncHRfcGxhbl90eXBlIjoicGx1cyJ9fQ.signature","expires_in":3600}"#,
                ))
            })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
