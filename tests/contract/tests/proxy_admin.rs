use std::{
    fs,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::RetrySafety;
use any2api_runtime::api::{
    ConfigPublisher, ProxyTestService, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    ReqwestTransportManager, TransportError, TransportErrorStage, TransportFailureScope,
    TransportManager, TransportProxy, TransportRequest, TransportResponse,
};
use async_trait::async_trait;
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

const DIRECT_ID: &str = "00000000-0000-0000-0000-000000000000";

#[tokio::test]
async fn proxy_admin_is_loopback_only_until_admin_authentication_exists() {
    let (_directory, app, _storage) = test_app().await;

    let (status, body) = request_json(
        app,
        Method::GET,
        "/api/admin/proxies",
        None,
        SocketAddr::from(([203, 0, 113, 10], 41000)),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");
}

#[tokio::test]
async fn proxy_crud_publishes_the_same_revision_to_storage_and_runtime() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, initial) = request_json(
        app.clone(),
        Method::GET,
        "/api/admin/proxies",
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["config_revision"], 1);
    assert_eq!(initial["global_proxy_id"], DIRECT_ID);
    assert_eq!(initial["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(initial["items"][0]["built_in"], true);

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "Hong Kong",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["config_revision"], 2);
    let proxy_id = created["items"]
        .as_array()
        .expect("proxy items")
        .iter()
        .find(|item| item["built_in"] == false)
        .and_then(|item| item["id"].as_str())
        .expect("custom proxy id")
        .to_owned();

    let (status, global) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/set-global"),
        Some(json!({ "expected_revision": 2 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(global["config_revision"], 3);
    assert_eq!(global["global_proxy_id"], proxy_id);

    let (status, disabled_global) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/proxies/{proxy_id}"),
        Some(json!({
            "expected_revision": 3,
            "name": "Hong Kong",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": false
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(disabled_global["error"]["code"], "proxy_in_use");

    let (status, updated) = request_json(
        app.clone(),
        Method::PATCH,
        &format!("/api/admin/proxies/{proxy_id}"),
        Some(json!({
            "expected_revision": 3,
            "name": "Hong Kong Primary",
            "kind": "socks5",
            "host": "socks.example.com",
            "port": 1080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config_revision"], 4);

    let (status, protected) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}?expected_revision=4"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(protected["error"]["code"], "proxy_in_use");

    let (status, direct_global) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{DIRECT_ID}/set-global"),
        Some(json!({ "expected_revision": 4 })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(direct_global["config_revision"], 5);

    let (status, deleted) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}?expected_revision=5"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["config_revision"], 6);
    assert_eq!(deleted["items"].as_array().map(Vec::len), Some(1));

    let stored = storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 6);
    assert_eq!(stored.proxies().profiles().len(), 1);

    let (status, health) = request_json(app, Method::GET, "/api/health", None, loopback).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["config_revision"], 6);
    assert_eq!(health["scheduler_epoch"], 5);
}

#[tokio::test]
async fn stale_revision_and_direct_mutation_return_structured_conflicts() {
    let (_directory, app, storage) = test_app().await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, _) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "First",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, stale) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "Stale",
            "kind": "socks5",
            "host": "stale.example.com",
            "port": 1080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["error"]["code"], "revision_conflict");

    let (status, direct) = request_json(
        app,
        Method::PATCH,
        &format!("/api/admin/proxies/{DIRECT_ID}"),
        Some(json!({
            "expected_revision": 2,
            "name": "DIRECT",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(direct["error"]["code"], "proxy_protected");

    let stored = storage
        .load_configuration()
        .await
        .expect("stored configuration");
    assert_eq!(stored.revision().get(), 2);
    assert_eq!(stored.proxies().profiles().len(), 2);
}

#[tokio::test]
async fn proxy_authentication_is_redacted_and_used_by_the_admin_probe() {
    let transport = Arc::new(CapturingProbeTransport::default());
    let (_directory, app, _storage) =
        test_app_with_transport(Arc::clone(&transport) as Arc<dyn TransportManager>).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, created) = request_json(
        app.clone(),
        Method::POST,
        "/api/admin/proxies",
        Some(json!({
            "expected_revision": 1,
            "name": "Authenticated Proxy",
            "kind": "http",
            "host": "proxy.example.com",
            "port": 8080,
            "enabled": true
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let proxy_id = custom_proxy_id(&created);

    let (status, authenticated) = request_json(
        app.clone(),
        Method::PUT,
        &format!("/api/admin/proxies/{proxy_id}/authentication"),
        Some(json!({
            "expected_revision": 2,
            "username": "proxy-user",
            "password": "proxy-password"
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let profile = custom_proxy(&authenticated);
    assert_eq!(profile["username"], "proxy-user");
    assert_eq!(profile["password_configured"], true);
    assert_eq!(profile["authentication_version"], 1);
    assert!(!authenticated.to_string().contains("proxy-password"));

    let (status, tested) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tested["reachable"], true);
    assert_eq!(tested["status_code"], 204);
    assert_eq!(tested["proxy_id"], proxy_id);
    assert_eq!(tested["config_revision"], 3);
    assert_eq!(tested["proxy_config_version"], 2);
    assert!(tested.get("provider_endpoint_id").is_none());
    assert!(tested.get("provider_endpoint_config_version").is_none());
    {
        let probes = transport.probes.lock().expect("captured probes");
        let first = probes.first().expect("first probe");
        assert_eq!(first.uri, "https://example.com/");
        assert_eq!(first.proxy_id, proxy_id);
        assert_eq!(first.proxy_username.as_deref(), Some("proxy-user"));
        assert!(first.headers.is_empty());
    }

    let (status, rejected) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rejected["reachable"], false);
    assert_eq!(rejected["status_code"], Value::Null);
    assert_eq!(rejected["error_stage"], "proxy_handshake");
    assert_eq!(rejected["failure_scope"], "proxy");
    assert_eq!(rejected["config_revision"], 3);
    {
        let probes = transport.probes.lock().expect("captured probes");
        assert_eq!(probes.len(), 2);
        assert_eq!(probes[1].uri, "https://example.com/");
    }

    let (status, arbitrary_target) = request_json(
        app.clone(),
        Method::POST,
        &format!("/api/admin/proxies/{proxy_id}/test"),
        Some(json!({
            "provider_endpoint_id": "11111111-1111-1111-1111-111111111111",
            "url": "http://127.0.0.1:1/private"
        })),
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(arbitrary_target["error"]["code"], "invalid_request");
    assert_eq!(transport.probes.lock().expect("captured probes").len(), 2);

    let (status, cleared) = request_json(
        app.clone(),
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}/authentication?expected_revision=3"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let profile = custom_proxy(&cleared);
    assert_eq!(profile["username"], Value::Null);
    assert_eq!(profile["password_configured"], false);
    assert_eq!(profile["authentication_version"], 2);

    let (status, repeated_clear) = request_json(
        app,
        Method::DELETE,
        &format!("/api/admin/proxies/{proxy_id}/authentication?expected_revision=4"),
        None,
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(repeated_clear["config_revision"], 4);
    let profile = custom_proxy(&repeated_clear);
    assert_eq!(profile["authentication_version"], 2);
}

async fn test_app() -> (tempfile::TempDir, Router, Arc<SqliteStore>) {
    test_app_with_transport(Arc::new(ReqwestTransportManager::default())).await
}

async fn test_app_with_transport(
    transport: Arc<dyn TransportManager>,
) -> (tempfile::TempDir, Router, Arc<SqliteStore>) {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
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
    let proxy_tests = Arc::new(ProxyTestService::new(transport));
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, public_requests).with_proxy_tests(proxy_tests),
        web_root,
    );

    (directory, app, storage)
}

fn custom_proxy_id(configuration: &Value) -> String {
    custom_proxy(configuration)["id"]
        .as_str()
        .expect("custom proxy id")
        .to_owned()
}

fn custom_proxy(configuration: &Value) -> &Value {
    configuration["items"]
        .as_array()
        .expect("proxy items")
        .iter()
        .find(|item| item["built_in"] == false)
        .expect("custom proxy")
}

#[derive(Default)]
struct CapturingProbeTransport {
    probes: Mutex<Vec<CapturedProbe>>,
}

struct CapturedProbe {
    proxy_id: String,
    proxy_username: Option<String>,
    uri: String,
    headers: axum::http::HeaderMap,
}

#[async_trait]
impl TransportManager for CapturingProbeTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, TransportError> {
        let mut probes = self.probes.lock().expect("captured probes");
        probes.push(CapturedProbe {
            proxy_id: proxy.profile().id().to_string(),
            proxy_username: proxy
                .credentials()
                .map(|credentials| credentials.username().to_owned()),
            uri: request.uri.to_string(),
            headers: request.headers,
        });
        if probes.len() == 1 {
            Ok(TransportResponse {
                status: StatusCode::NO_CONTENT,
                headers: axum::http::HeaderMap::new(),
                body: Box::pin(futures_util::stream::empty()),
                read_failure_scope: TransportFailureScope::Endpoint,
            })
        } else {
            Err(TransportError::new(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                RetrySafety::DefinitelyNotSent,
                "configured proxy authentication was rejected",
            ))
        }
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
