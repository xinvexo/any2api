use std::{net::SocketAddr, sync::Arc};

use any2api_contract_tests::TestApplication;
use any2api_runtime::api::OAuthService;
use any2api_server::api::{
    AdminAuthService, AdminCredentialStore, AdminCredentialStoreError, AppState,
    StoredAdminPasswordHash,
};
use any2api_storage::api::{AdminCredentialRepository, ConfigurationRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::SET_COOKIE},
};
use http_body_util::BodyExt;
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery staple";

#[tokio::test]
async fn multipart_import_accepts_multiple_files_and_multi_account_documents() {
    let context = TestContext::new(false).await;
    let first = serde_json::to_vec(&json!({
        "accounts": [
            {
                "name": "Shared",
                "platform": "openai",
                "type": "oauth",
                "credentials": {
                    "access_token": "codex-secret-one",
                    "refresh_token": "codex-refresh-one",
                    "email": "codex@example.com"
                },
                "concurrency": 99
            },
            {
                "name": "Claude Imported",
                "platform": "anthropic",
                "type": "oauth",
                "credentials": {"access_token": "claude-secret"}
            }
        ]
    }))
    .expect("JSON");
    let second = br#"{"type":"codex","name":"Shared","access_token":"codex-secret-two"}"#;
    let (content_type, body) = multipart(&[
        ("source-secret-name.json", first.as_slice()),
        ("second.json", second.as_slice()),
    ]);

    let response = request(
        &context.app,
        Method::POST,
        "/api/admin/oauth/import",
        Body::from(body),
        &[("content-type", &content_type)],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    let value = response.json();
    assert_eq!(value["imported_count"], 3);
    assert_eq!(value["config_revision"], 2);
    assert_eq!(value["items"].as_array().map(Vec::len), Some(3));
    assert_eq!(value["items"][0]["requests_per_minute"], Value::Null);
    assert_eq!(value["items"][0]["enabled"], true);
    let response_text = String::from_utf8(response.body.to_vec()).expect("UTF-8 response");
    for forbidden in [
        "source-secret-name.json",
        "codex-secret-one",
        "codex-refresh-one",
        "claude-secret",
        "oauth_json",
    ] {
        assert!(!response_text.contains(forbidden));
    }

    let stored = context
        .storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 2);
    assert_eq!(stored.oauth_accounts().accounts().len(), 3);
    let labels = stored
        .oauth_accounts()
        .accounts()
        .iter()
        .map(|account| account.label())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Shared"));
    assert!(labels.contains(&"Shared (2)"));
    let materials = stored.into_parts().oauth_account_materials.into_entries();
    assert_eq!(materials.len(), 3);
    for material in materials {
        let document = material.into_document().into_bytes();
        let document: Value =
            serde_json::from_slice(document.expose_secret()).expect("canonical document");
        assert_eq!(document.as_object().expect("document object").len(), 5);
        assert!(document.get("access_token").is_some());
        assert!(document.get("type").is_none());
        assert!(document.get("credentials").is_none());
        assert!(document.get("concurrency").is_none());
    }
}

#[tokio::test]
async fn invalid_later_file_and_upload_limits_leave_the_batch_uncommitted() {
    let context = TestContext::new(false).await;
    let valid = br#"{"type":"codex","access_token":"valid-secret"}"#;
    let invalid = br#"{"type":"claude","access_token":""}"#;
    let (content_type, body) = multipart(&[("one.json", valid), ("two.json", invalid)]);
    let response = request(
        &context.app,
        Method::POST,
        "/api/admin/oauth/import",
        Body::from(body),
        &[("content-type", &content_type)],
    )
    .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        response.json()["error"]["code"],
        "oauth_import_invalid_account"
    );
    assert!(
        response.json()["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("file 2, account 1"))
    );
    assert_initial(&context.storage).await;

    let files = (0..33)
        .map(|index| (format!("{index}.json"), valid.as_slice()))
        .collect::<Vec<_>>();
    let borrowed = files
        .iter()
        .map(|(name, bytes)| (name.as_str(), *bytes))
        .collect::<Vec<_>>();
    let (content_type, body) = multipart(&borrowed);
    let response = request(
        &context.app,
        Method::POST,
        "/api/admin/oauth/import",
        Body::from(body),
        &[("content-type", &content_type)],
    )
    .await;
    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        response.json()["error"]["code"],
        "oauth_import_too_many_files"
    );
    assert_initial(&context.storage).await;
}

#[tokio::test]
async fn duplicate_provider_identity_returns_conflict_without_committing() {
    let context = TestContext::new(false).await;
    let duplicate = serde_json::to_vec(&json!({
        "accounts": [
            {
                "platform": "openai",
                "type": "oauth",
                "credentials": {
                    "access_token": "first-secret",
                    "email": "same@example.com"
                }
            },
            {
                "platform": "openai",
                "type": "oauth",
                "credentials": {
                    "access_token": "second-secret",
                    "email": "SAME@example.com"
                }
            }
        ]
    }))
    .expect("JSON");
    let (content_type, body) = multipart(&[("duplicates.json", duplicate.as_slice())]);

    let response = request(
        &context.app,
        Method::POST,
        "/api/admin/oauth/import",
        Body::from(body),
        &[("content-type", &content_type)],
    )
    .await;

    assert_eq!(response.status, StatusCode::CONFLICT);
    let body = response.json();
    assert_eq!(body["error"]["code"], "oauth_account_identity_conflict");
    let response_text = String::from_utf8(response.body.to_vec()).expect("UTF-8 response");
    for forbidden in ["same@example.com", "first-secret", "second-secret"] {
        assert!(!response_text.contains(forbidden));
    }
    assert_initial(&context.storage).await;
}

#[tokio::test]
async fn multipart_import_requires_the_admin_csrf_token() {
    let context = TestContext::new(true).await;
    let setup = request_json(
        &context.app,
        "/api/admin/auth/setup",
        json!({
            "setup_token": context.setup_token.as_deref().expect("setup token"),
            "password": PASSWORD
        }),
        &[],
    )
    .await;
    assert_eq!(setup.status, StatusCode::OK);
    let cookie = setup
        .headers
        .get(SET_COOKIE)
        .expect("session cookie")
        .to_str()
        .expect("cookie text")
        .split(';')
        .next()
        .expect("cookie pair")
        .to_owned();
    let csrf = setup.json()["csrf_token"]
        .as_str()
        .expect("CSRF token")
        .to_owned();
    let file = br#"{"type":"claude","access_token":"claude-secret"}"#;

    let (content_type, body) = multipart(&[("claude.json", file)]);
    let missing = request(
        &context.app,
        Method::POST,
        "/api/admin/oauth/import",
        Body::from(body),
        &[("content-type", &content_type), ("cookie", &cookie)],
    )
    .await;
    assert_eq!(missing.status, StatusCode::FORBIDDEN);
    assert_eq!(missing.json()["error"]["code"], "admin_csrf_invalid");
    assert_initial(&context.storage).await;

    let (content_type, body) = multipart(&[("claude.json", file)]);
    let accepted = request(
        &context.app,
        Method::POST,
        "/api/admin/oauth/import",
        Body::from(body),
        &[
            ("content-type", &content_type),
            ("cookie", &cookie),
            ("x-csrf-token", &csrf),
        ],
    )
    .await;
    assert_eq!(accepted.status, StatusCode::OK);
    assert_eq!(accepted.json()["imported_count"], 1);
}

struct TestContext {
    _directory: TempDir,
    app: Router,
    storage: Arc<SqliteStore>,
    setup_token: Option<String>,
}

impl TestContext {
    async fn new(with_auth: bool) -> Self {
        let fixture = TestApplication::new().await;
        let storage = fixture.storage();
        let runtime = fixture.runtime();
        let oauth = Arc::new(OAuthService::new(
            fixture.components().provider_registry_handle(),
            fixture.components().transport_manager(),
            fixture.publisher(),
            Arc::clone(&storage),
        ));
        let (directory, app, _fixture_storage, setup_token) = if with_auth {
            let auth = Arc::new(
                AdminAuthService::load(
                    Arc::new(TestAdminStore {
                        storage: Arc::clone(&storage),
                    }),
                    runtime.lifecycle(),
                )
                .await
                .expect("admin auth"),
            );
            let setup_token = auth.setup_token().await;
            let state = AppState::new(
                fixture.snapshots(),
                Arc::clone(&runtime),
                fixture.publisher(),
                fixture.components().service(),
                auth,
            )
            .with_oauth(oauth);
            let (directory, app, storage) = fixture.into_raw_router_with_state(state);
            (directory, app, storage, setup_token)
        } else {
            let state = fixture.state().with_oauth(oauth);
            let (directory, app, storage) = fixture.into_router_with_state(state);
            (directory, app, storage, None)
        };
        Self {
            _directory: directory,
            app,
            storage,
            setup_token,
        }
    }
}

struct TestAdminStore {
    storage: Arc<SqliteStore>,
}

#[async_trait::async_trait]
impl AdminCredentialStore for TestAdminStore {
    async fn load(&self) -> Result<Option<StoredAdminPasswordHash>, AdminCredentialStoreError> {
        self.storage
            .load_admin_credential()
            .await
            .map(|value| {
                value.map(|value| StoredAdminPasswordHash::new(value.password_hash().to_owned()))
            })
            .map_err(|error| Box::new(error) as AdminCredentialStoreError)
    }

    async fn initialize(&self, password_hash: &str) -> Result<bool, AdminCredentialStoreError> {
        self.storage
            .initialize_admin_credential(password_hash)
            .await
            .map_err(|error| Box::new(error) as AdminCredentialStoreError)
    }

    async fn replace(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, AdminCredentialStoreError> {
        self.storage
            .replace_admin_credential(expected_password_hash, new_password_hash)
            .await
            .map_err(|error| Box::new(error) as AdminCredentialStoreError)
    }
}

struct TestResponse {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    body: bytes::Bytes,
}

impl TestResponse {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).expect("response JSON")
    }
}

async fn request_json(
    app: &Router,
    uri: &str,
    body: Value,
    headers: &[(&str, &str)],
) -> TestResponse {
    let bytes = serde_json::to_vec(&body).expect("request JSON");
    let mut headers = headers.to_vec();
    headers.push(("content-type", "application/json"));
    request(app, Method::POST, uri, Body::from(bytes), &headers).await
}

async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    body: Body,
    headers: &[(&str, &str)],
) -> TestResponse {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000))));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(builder.body(body).expect("request"))
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("response body")
        .to_bytes();
    TestResponse {
        status,
        headers,
        body,
    }
}

fn multipart(files: &[(&str, &[u8])]) -> (String, Vec<u8>) {
    let boundary = "any2api-oauth-import-boundary";
    let mut body = Vec::new();
    for (name, bytes) in files {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"files\"; filename=\"{name}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/json\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

async fn assert_initial(storage: &SqliteStore) {
    let stored = storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 1);
    assert!(stored.oauth_accounts().accounts().is_empty());
}
