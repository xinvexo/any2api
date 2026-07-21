use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::SettingKey;
use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{
    AdminAuthService, AdminCredentialStore, AdminCredentialStoreError, AdminNetworkPolicy,
    AppState, StoredAdminPasswordHash, build_router,
};
use any2api_storage::api::{AdminCredentialRepository, ConfigurationRepository, SqliteStore};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{
        HeaderMap, Method, Request, StatusCode,
        header::{CONTENT_TYPE, SET_COOKIE},
    },
};
use http_body_util::BodyExt;
use ipnet::IpNet;
use serde_json::{Value, json};
use tempfile::tempdir;
use tower::ServiceExt;

const PASSWORD: &str = "correct horse battery staple";

#[tokio::test]
async fn setup_login_csrf_remote_http_logout_and_restart_follow_the_admin_contract() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let (app, setup_token) = build_test_app(
        Arc::clone(&storage),
        web_root.clone(),
        AdminNetworkPolicy::default(),
    )
    .await;
    let setup_token = setup_token.expect("setup token");
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let remote = SocketAddr::from(([203, 0, 113, 10], 41000));

    let response = request(
        &app,
        Method::GET,
        "/api/admin/auth/session",
        None,
        remote,
        &[],
    )
    .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json()["error"]["code"], "admin_remote_disabled");

    let response = request(
        &app,
        Method::GET,
        "/api/admin/auth/session",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["initialized"], false);
    assert_eq!(response.json()["authenticated"], false);

    let response = request(
        &app,
        Method::GET,
        "/api/admin/settings",
        None,
        loopback,
        &[],
    )
    .await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.json()["error"]["code"], "admin_session_required");

    let setup = request(
        &app,
        Method::POST,
        "/api/admin/auth/setup",
        Some(json!({ "setup_token": setup_token.clone(), "password": PASSWORD })),
        loopback,
        &[],
    )
    .await;
    assert_eq!(setup.status, StatusCode::OK);
    let cookie = setup.cookie();
    assert!(cookie.contains("HttpOnly"));
    assert!(cookie.contains("SameSite=Strict"));
    assert!(cookie.contains("Path=/api/admin"));
    assert!(!cookie.contains("Secure"));
    let cookie_pair = cookie.split(';').next().expect("cookie pair").to_owned();
    let csrf = setup.json()["csrf_token"]
        .as_str()
        .expect("csrf token")
        .to_owned();

    let response = request(
        &app,
        Method::GET,
        "/api/admin/settings",
        None,
        loopback,
        &[("cookie", &cookie_pair)],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response
            .headers
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(response.json()["items"].as_array().map(Vec::len), Some(47));

    let response = request(
        &app,
        Method::PATCH,
        "/api/admin/settings/admin.remote_enabled",
        Some(json!({ "expected_revision": 1, "value": true })),
        loopback,
        &[("cookie", &cookie_pair)],
    )
    .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert_eq!(response.json()["error"]["code"], "admin_csrf_invalid");

    let response = request(
        &app,
        Method::PATCH,
        "/api/admin/settings/admin.remote_enabled",
        Some(json!({ "expected_revision": 1, "value": true })),
        loopback,
        &[("cookie", &cookie_pair), ("x-csrf-token", &csrf)],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        find_setting(response.json(), SettingKey::AdminRemoteEnabled.as_str())["effective_value"],
        true
    );

    let response = request(
        &app,
        Method::GET,
        "/api/admin/auth/session",
        None,
        remote,
        &[],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["plaintext_http_warning"], true);
    assert_eq!(response.json()["authenticated"], false);

    let login = request(
        &app,
        Method::POST,
        "/api/admin/auth/login",
        Some(json!({ "password": PASSWORD })),
        remote,
        &[],
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.json()["plaintext_http_warning"], true);
    assert!(!login.cookie().contains("Secure"));
    let remote_cookie = login
        .cookie()
        .split(';')
        .next()
        .expect("remote cookie")
        .to_owned();
    let remote_csrf = login.json()["csrf_token"]
        .as_str()
        .expect("remote csrf")
        .to_owned();

    let (restarted, _) = build_test_app(
        Arc::clone(&storage),
        web_root,
        AdminNetworkPolicy::default(),
    )
    .await;
    let response = request(
        &restarted,
        Method::GET,
        "/api/admin/auth/session",
        None,
        remote,
        &[("cookie", &remote_cookie)],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["initialized"], true);
    assert_eq!(response.json()["authenticated"], false);

    let logout = request(
        &app,
        Method::POST,
        "/api/admin/auth/logout",
        None,
        remote,
        &[("cookie", &remote_cookie), ("x-csrf-token", &remote_csrf)],
    )
    .await;
    assert_eq!(logout.status, StatusCode::NO_CONTENT);
    assert!(logout.cookie().contains("Max-Age=0"));
    let response = request(
        &app,
        Method::GET,
        "/api/admin/settings",
        None,
        remote,
        &[("cookie", &remote_cookie)],
    )
    .await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn trusted_proxy_cidr_controls_forwarded_https_and_secure_cookie() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let (app, setup_token) = build_test_app(
        storage,
        web_root,
        AdminNetworkPolicy::new(vec!["127.0.0.0/8".parse::<IpNet>().expect("cidr")]),
    )
    .await;
    let setup_token = setup_token.expect("setup token");
    let proxy = SocketAddr::from(([127, 0, 0, 1], 41000));
    let missing_forwarded = request(
        &app,
        Method::GET,
        "/api/admin/auth/session",
        None,
        proxy,
        &[],
    )
    .await;
    assert_eq!(missing_forwarded.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        missing_forwarded.json()["error"]["code"],
        "admin_invalid_forwarded_headers"
    );
    let spoofed_loopback = request(
        &app,
        Method::POST,
        "/api/admin/auth/setup",
        Some(json!({ "setup_token": setup_token.clone(), "password": PASSWORD })),
        proxy,
        &[
            ("x-forwarded-for", "127.0.0.1, 203.0.113.8"),
            ("x-forwarded-proto", "http"),
        ],
    )
    .await;
    assert_eq!(spoofed_loopback.status, StatusCode::FORBIDDEN);
    assert_eq!(
        spoofed_loopback.json()["error"]["code"],
        "admin_remote_disabled"
    );
    let setup = request(
        &app,
        Method::POST,
        "/api/admin/auth/setup",
        Some(json!({ "setup_token": setup_token, "password": PASSWORD })),
        proxy,
        &[
            ("x-forwarded-for", "127.0.0.1"),
            ("x-forwarded-proto", "http"),
        ],
    )
    .await;
    let cookie_pair = setup.cookie().split(';').next().expect("cookie").to_owned();
    let csrf = setup.json()["csrf_token"]
        .as_str()
        .expect("csrf")
        .to_owned();
    let updated = request(
        &app,
        Method::PATCH,
        "/api/admin/settings/admin.remote_enabled",
        Some(json!({ "expected_revision": 1, "value": true })),
        proxy,
        &[
            ("cookie", &cookie_pair),
            ("x-csrf-token", &csrf),
            ("x-forwarded-for", "127.0.0.1"),
            ("x-forwarded-proto", "http"),
        ],
    )
    .await;
    assert_eq!(updated.status, StatusCode::OK);

    let login = request(
        &app,
        Method::POST,
        "/api/admin/auth/login",
        Some(json!({ "password": PASSWORD })),
        proxy,
        &[
            ("x-forwarded-for", "203.0.113.8"),
            ("x-forwarded-proto", "https"),
        ],
    )
    .await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.json()["secure_transport"], true);
    assert_eq!(login.json()["through_trusted_proxy"], true);
    assert_eq!(login.json()["plaintext_http_warning"], false);
    assert!(login.cookie().contains("Secure"));
}

async fn build_test_app(
    storage: Arc<SqliteStore>,
    web_root: std::path::PathBuf,
    network: AdminNetworkPolicy,
) -> (Router, Option<String>) {
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
    let auth = Arc::new(
        AdminAuthService::load(Arc::new(TestAdminStore { storage }))
            .await
            .expect("admin auth"),
    );
    let setup_token = auth.setup_token().await;
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    (
        build_router(
            AppState::new(snapshots, runtime, publisher, public_requests)
                .with_admin_auth(auth, network),
            web_root,
        ),
        setup_token,
    )
}

struct TestAdminStore {
    storage: Arc<SqliteStore>,
}

#[async_trait]
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
}

struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Option<Value>,
}

impl TestResponse {
    fn json(&self) -> &Value {
        self.body.as_ref().expect("JSON response")
    }

    fn cookie(&self) -> &str {
        self.headers
            .get(SET_COOKIE)
            .expect("set-cookie header")
            .to_str()
            .expect("set-cookie text")
    }
}

async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    remote: SocketAddr,
    headers: &[(&str, &str)],
) -> TestResponse {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .extension(ConnectInfo(remote));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let body = if let Some(value) = body {
        builder = builder.header(CONTENT_TYPE, "application/json");
        Body::from(serde_json::to_vec(&value).expect("request JSON"))
    } else {
        Body::empty()
    };
    let response = app
        .clone()
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
    let body = (!bytes.is_empty()).then(|| serde_json::from_slice(&bytes).expect("response JSON"));
    TestResponse {
        status,
        headers,
        body,
    }
}

fn find_setting<'a>(response: &'a Value, key: &str) -> &'a Value {
    response["items"]
        .as_array()
        .expect("setting items")
        .iter()
        .find(|item| item["key"] == key)
        .expect("setting item")
}
