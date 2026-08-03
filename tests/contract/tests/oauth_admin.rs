use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use any2api_contract_tests::TestApplication;
use any2api_domain::{
    OAuthAccountDraft, OAuthAccountId, ProviderKind, ProxyProfileId, RequestsPerMinute, RetrySafety,
};
use any2api_provider::{CodexDriver, GrokDriver, api::ProviderRegistry};
use any2api_runtime::api::{ConfigPublisher, OAuthService};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument, SqliteStore};
use any2api_transport::api::{
    TransportError, TransportErrorStage, TransportFailureScope, TransportManager, TransportProxy,
    TransportRequest, TransportResponse,
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
use tower::ServiceExt;

#[tokio::test]
async fn oauth_start_requires_an_admin_session_and_does_not_publish_configuration() {
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
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "admin_session_required");

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
    assert_eq!(start["flow"], "authorization_code");
    assert!(
        start["authorization_url"]
            .as_str()
            .is_some_and(|url| url.starts_with("https://auth.openai.com/"))
    );
    assert_eq!(start["redirect_uri"], "http://localhost:1455/auth/callback");
    assert_eq!(start["expires_in_seconds"], 600);

    let (status, grok_start) = request_json(
        app,
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "grok"})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grok_start["provider"], "grok");
    assert_eq!(grok_start["flow"], "device_code");
    assert_eq!(grok_start["user_code"], "ABCD-1234");
    assert_eq!(
        grok_start["verification_uri"],
        "https://accounts.x.ai/oauth2/device"
    );
    assert_eq!(
        grok_start["verification_uri_complete"],
        "https://accounts.x.ai/oauth2/device?user_code=ABCD-1234"
    );
    assert_eq!(grok_start["expires_in_seconds"], 1_800);
    assert_eq!(grok_start["poll_interval_seconds"], 5);
    assert!(grok_start.get("device_code").is_none());
    assert!(grok_start.get("redirect_uri").is_none());

    let configuration = storage.load_configuration().await.expect("configuration");
    assert_eq!(configuration.revision().get(), 1);
    assert!(configuration.provider_endpoints().endpoints().is_empty());
    drop(directory);
}

#[tokio::test]
async fn grok_device_poll_activates_sqlite_account_without_exposing_tokens() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (status, start) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "grok"})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let session_id = start["session_id"].as_str().expect("device session id");

    let response = request(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/device/poll",
        Some(json!({"session_id": session_id})),
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
    let poll: Value = serde_json::from_slice(&response.body).expect("poll response");
    assert_eq!(poll["status"], "complete");
    assert_eq!(poll["account"]["provider"], "grok");
    assert_eq!(poll["account"]["safe_account_email"], "grok@example.com");
    assert_eq!(poll["account"]["selected_model_count"], 7);
    let text = String::from_utf8(response.body.to_vec()).expect("UTF-8 response");
    assert!(!text.contains("device-secret"));
    assert!(!text.contains("grok-access-token"));
    assert!(!text.contains("grok-refresh-token"));

    let stored = storage.load_configuration().await.expect("configuration");
    let account = stored
        .oauth_accounts()
        .accounts()
        .first()
        .expect("persisted Grok OAuth account");
    assert_eq!(account.provider_kind(), any2api_domain::ProviderKind::Grok);
    assert_eq!(account.models().len(), 7);
    let mut materials = stored.into_parts().oauth_account_materials.into_entries();
    let document = materials
        .pop()
        .expect("Grok OAuth document")
        .into_document()
        .into_bytes();
    let document: Value =
        serde_json::from_slice(document.expose_secret()).expect("stored Grok document");
    assert_eq!(document["type"], "grok");
    assert_eq!(document["access_token"], "grok-access-token");

    let (status, replay) = request_json(
        app,
        Method::POST,
        "/api/admin/oauth/device/poll",
        Some(json!({"session_id": session_id})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(replay["error"]["code"], "oauth_session_invalid");
}

#[tokio::test]
async fn transient_grok_poll_failure_preserves_the_device_session() {
    let transport = Arc::new(TokenTransport {
        fail_next_grok_poll: AtomicBool::new(true),
        ..TokenTransport::default()
    });
    let (_directory, app, _storage) =
        test_app_with_transport(transport as Arc<dyn TransportManager>).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (status, start) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "grok"})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let session_id = start["session_id"].as_str().expect("device session id");

    let (status, failed) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/device/poll",
        Some(json!({"session_id": session_id})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(failed["error"]["code"], "oauth_token_exchange_failed");

    let (status, pending) = request_json(
        app,
        Method::POST,
        "/api/admin/oauth/device/poll",
        Some(json!({"session_id": session_id})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pending["status"], "pending");
    assert_eq!(pending["retry_after_seconds"], 5);
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
    assert_eq!(activation["requests_per_minute"], Value::Null);
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
    assert_eq!(account.requests_per_minute(), None);
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

#[tokio::test]
async fn repeated_login_reauthorizes_the_same_account_and_preserves_local_configuration() {
    let transport = Arc::new(RelLoginTokenTransport::default());
    let (_directory, app, storage, publisher) =
        test_app_with_transport_and_publisher(transport).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, first) = complete_codex_login(app.clone(), loopback).await;
    assert_eq!(status, StatusCode::OK);
    let stored = storage.load_configuration().await.expect("configuration");
    let account = stored
        .oauth_accounts()
        .accounts()
        .first()
        .expect("first OAuth account");
    let account_id = account.id();
    assert_eq!(first["account_id"], account_id.to_string());

    let updated = publisher
        .update_oauth_account(
            stored.revision(),
            account_id,
            account.config_version(),
            OAuthAccountDraft::new(
                "Preserved account",
                Some(RequestsPerMinute::new(17).expect("RPM")),
                false,
            )
            .expect("account draft"),
        )
        .await
        .expect("update local account settings");
    let account = updated
        .oauth_accounts()
        .get(account_id)
        .expect("updated OAuth account");
    publisher
        .set_oauth_account_models(
            updated.revision(),
            account_id,
            account.config_version(),
            vec!["gpt-5.3-codex-spark".into()],
        )
        .await
        .expect("narrow selected models");

    let (status, second) = complete_codex_login(app, loopback).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["account_id"], account_id.to_string());
    assert_eq!(second["label"], "Preserved account");
    assert_eq!(second["requests_per_minute"], 17);
    assert_eq!(second["enabled"], false);
    assert_eq!(second["selected_model_count"], 1);
    assert_eq!(second["config_version"], 3);

    let stored = storage.load_configuration().await.expect("configuration");
    assert_eq!(stored.oauth_accounts().accounts().len(), 1);
    let account = stored
        .oauth_accounts()
        .get(account_id)
        .expect("reauthorized OAuth account");
    assert_eq!(account.label(), "Preserved account");
    assert_eq!(
        account.requests_per_minute().map(|value| value.get()),
        Some(17)
    );
    assert!(!account.enabled());
    assert_eq!(account.config_version(), 3);
    assert_eq!(account.models().len(), 1);
    assert_eq!(account.models()[0].as_str(), "gpt-5.3-codex-spark");
    assert_eq!(account.token_version(), 2);

    let document = stored
        .into_parts()
        .oauth_account_materials
        .into_entries()
        .into_iter()
        .find(|material| material.account_id() == account_id)
        .expect("reauthorized material")
        .into_document()
        .into_bytes();
    let document: Value =
        serde_json::from_slice(document.expose_secret()).expect("stored OAuth document");
    assert_eq!(document["access_token"], "access-token-2");
}

#[tokio::test]
async fn same_email_with_different_stable_account_ids_creates_separate_accounts() {
    let transport = Arc::new(DifferentIdentityLoginTokenTransport::default());
    let (_directory, app, storage, _publisher) =
        test_app_with_transport_and_publisher(transport).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, first) = complete_codex_login(app.clone(), loopback).await;
    assert_eq!(status, StatusCode::OK);
    let (status, second) = complete_codex_login(app, loopback).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(first["account_id"], second["account_id"]);
    assert_eq!(first["label"], "person@example.com");
    assert_eq!(second["label"], "person@example.com (2)");

    let stored = storage.load_configuration().await.expect("configuration");
    assert_eq!(stored.oauth_accounts().accounts().len(), 2);
    assert!(
        stored
            .oauth_accounts()
            .accounts()
            .iter()
            .all(|account| account.token_version() == 1)
    );
}

#[tokio::test]
async fn login_rejects_ambiguous_existing_provider_identity() {
    let transport = Arc::new(RelLoginTokenTransport::default());
    let (_directory, app, storage, publisher) =
        test_app_with_transport_and_publisher(transport).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    for (label, access_token) in [
        ("Imported duplicate A", "imported-access-a"),
        ("Imported duplicate B", "imported-access-b"),
    ] {
        publisher
            .activate_oauth_account(
                OAuthAccountId::new(),
                ProviderKind::Codex,
                OAuthAccountDraft::new(label, None, true).expect("OAuth account draft"),
                Some("person@example.com".into()),
                None,
                vec!["gpt-5.5".into()],
                codex_account_document(access_token, "account-123"),
            )
            .await
            .expect("create duplicate identity fixture");
    }

    let (status, body) = complete_codex_login(app, loopback).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "oauth_account_identity_conflict");

    let stored = storage.load_configuration().await.expect("configuration");
    assert_eq!(stored.oauth_accounts().accounts().len(), 2);
    assert!(
        stored
            .oauth_accounts()
            .accounts()
            .iter()
            .all(|account| account.token_version() == 1)
    );
}

async fn test_app() -> (tempfile::TempDir, Router, Arc<SqliteStore>) {
    test_app_with_transport(Arc::new(TokenTransport::default())).await
}

async fn test_app_with_transport(
    token_transport: Arc<dyn TransportManager>,
) -> (tempfile::TempDir, Router, Arc<SqliteStore>) {
    let (directory, app, storage, _publisher) =
        test_app_with_transport_and_publisher(token_transport).await;
    (directory, app, storage)
}

async fn test_app_with_transport_and_publisher(
    token_transport: Arc<dyn TransportManager>,
) -> (
    tempfile::TempDir,
    Router,
    Arc<SqliteStore>,
    Arc<ConfigPublisher>,
) {
    let fixture = TestApplication::new().await;
    let storage = fixture.storage();
    let publisher = fixture.publisher();
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");
    let oauth = Arc::new(OAuthService::new(
        Arc::new(providers),
        token_transport,
        Arc::clone(&publisher),
    ));
    let state = fixture.state().with_oauth(oauth);
    let (directory, app, _fixture_storage) = fixture.into_router_with_state(state);
    (directory, app, storage, publisher)
}

async fn complete_codex_login(app: Router, loopback: SocketAddr) -> (StatusCode, Value) {
    let (status, start) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/oauth/start",
        Some(json!({"provider": "codex"})),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let session_id = start["session_id"].as_str().expect("session id");
    let authorization_url = start["authorization_url"]
        .as_str()
        .expect("authorization URL");
    let state = url::Url::parse(authorization_url)
        .expect("authorization URL")
        .query_pairs()
        .find_map(|(key, value)| (key == "state").then(|| value.into_owned()))
        .expect("OAuth state");
    request_json(
        app,
        Method::POST,
        "/api/admin/oauth/exchange",
        Some(json!({
            "session_id": session_id,
            "callback_url": format!(
                "http://localhost:1455/auth/callback?code=authorization-code&state={state}"
            )
        })),
        loopback,
    )
    .await
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
    codex_exchanged: AtomicBool,
    grok_polled: AtomicBool,
    fail_next_grok_poll: AtomicBool,
}

#[derive(Default)]
struct RelLoginTokenTransport {
    exchanges: AtomicUsize,
}

#[derive(Default)]
struct DifferentIdentityLoginTokenTransport {
    exchanges: AtomicUsize,
}

#[async_trait]
impl TransportManager for RelLoginTokenTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        assert_eq!(request.uri.host(), Some("auth.openai.com"));
        assert_eq!(request.uri.path(), "/oauth/token");
        let sequence = self.exchanges.fetch_add(1, Ordering::SeqCst) + 1;
        let body = bytes::Bytes::from(format!(
            r#"{{"access_token":"access-token-{sequence}","refresh_token":"refresh-token-{sequence}","id_token":"header.eyJlbWFpbCI6InBlcnNvbkBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NvdW50LTEyMyIsImNoYXRncHRfcGxhbl90eXBlIjoicGx1cyJ9fQ.signature","expires_in":3600}}"#
        ));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async { Ok(body) })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

#[async_trait]
impl TransportManager for DifferentIdentityLoginTokenTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        assert_eq!(request.uri.host(), Some("auth.openai.com"));
        assert_eq!(request.uri.path(), "/oauth/token");
        let sequence = self.exchanges.fetch_add(1, Ordering::SeqCst) + 1;
        let body = bytes::Bytes::from(format!(
            r#"{{"access_token":"access-token-{sequence}","refresh_token":"refresh-token-{sequence}","account_id":"account-{sequence}","email":"person@example.com","expires_in":3600}}"#
        ));
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async { Ok(body) })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

#[async_trait]
impl TransportManager for TokenTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        assert_eq!(proxy.profile().id(), ProxyProfileId::DIRECT);
        let body = match (request.uri.host(), request.uri.path()) {
            (Some("auth.openai.com"), "/oauth/token") => {
                assert!(!self.codex_exchanged.swap(true, Ordering::SeqCst));
                bytes::Bytes::from_static(
                    br#"{"access_token":"access-token","refresh_token":"refresh-token","id_token":"header.eyJlbWFpbCI6InBlcnNvbkBleGFtcGxlLmNvbSIsImh0dHBzOi8vYXBpLm9wZW5haS5jb20vYXV0aCI6eyJjaGF0Z3B0X2FjY291bnRfaWQiOiJhY2NvdW50LTEyMyIsImNoYXRncHRfcGxhbl90eXBlIjoicGx1cyJ9fQ.signature","expires_in":3600}"#,
                )
            }
            (Some("auth.x.ai"), "/oauth2/device/code") => {
                let form: std::collections::HashMap<_, _> =
                    url::form_urlencoded::parse(&request.body)
                        .into_owned()
                        .collect();
                assert_eq!(
                    form.get("client_id").map(String::as_str),
                    Some("b1a00492-073a-47ea-816f-4c329264a828")
                );
                assert_eq!(
                    form.get("scope").map(String::as_str),
                    Some("openid profile email offline_access grok-cli:access api:access")
                );
                bytes::Bytes::from_static(
                    br#"{"device_code":"device-secret","user_code":"ABCD-1234","verification_uri":"https://accounts.x.ai/oauth2/device","verification_uri_complete":"https://accounts.x.ai/oauth2/device?user_code=ABCD-1234","expires_in":1800,"interval":5}"#,
                )
            }
            (Some("auth.x.ai"), "/oauth2/token") => {
                if self.fail_next_grok_poll.swap(false, Ordering::SeqCst) {
                    return Err(TransportError::new(
                        TransportErrorStage::AwaitHeaders,
                        TransportFailureScope::Endpoint,
                        RetrySafety::DefinitelyNotSent,
                        "temporary device token failure",
                    ));
                }
                assert!(!self.grok_polled.swap(true, Ordering::SeqCst));
                let form: std::collections::HashMap<_, _> =
                    url::form_urlencoded::parse(&request.body)
                        .into_owned()
                        .collect();
                assert_eq!(
                    form.get("grant_type").map(String::as_str),
                    Some("urn:ietf:params:oauth:grant-type:device_code")
                );
                assert_eq!(
                    form.get("device_code").map(String::as_str),
                    Some("device-secret")
                );
                bytes::Bytes::from_static(
                    br#"{"access_token":"grok-access-token","refresh_token":"grok-refresh-token","id_token":"header.eyJlbWFpbCI6Imdyb2tAZXhhbXBsZS5jb20iLCJzdWIiOiJncm9rLXN1YmplY3QifQ.signature","expires_in":3600}"#,
                )
            }
            target => panic!("unexpected OAuth request target: {target:?}"),
        };
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::once(async { Ok(body) })),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}

fn codex_account_document(access_token: &str, account_id: &str) -> OAuthAccountDocument {
    let bytes = format!(
        r#"{{"type":"codex","access_token":"{access_token}","refresh_token":"refresh-token","account_id":"{account_id}","email":"person@example.com"}}"#
    )
    .into_bytes();
    OAuthAccountDocument::new(ProviderKind::Codex, bytes.into()).expect("OAuth document")
}
