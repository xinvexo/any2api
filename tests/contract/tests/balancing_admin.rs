use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind, ProxyAddress,
    ProxyDraft, ProxyKind, ProxyProfileId,
};
use any2api_runtime::api::{
    ConfigPublisher, ProviderApiKeySecret, PublishedSnapshot, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode, header::CACHE_CONTROL},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn balancing_admin_exposes_live_capacity_and_compiled_queue_policy() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("storage"),
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
    let proxy_id = ProxyProfileId::new();
    let proxy = publisher
        .create_proxy(
            ConfigRevision::INITIAL,
            proxy_id,
            ProxyDraft::new(
                "Disabled Proxy",
                ProxyKind::Http,
                ProxyAddress::new("proxy.example.com", 8080).expect("proxy address"),
                false,
            )
            .expect("proxy draft"),
        )
        .await
        .expect("proxy publish");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = publisher
        .create_provider_endpoint(
            proxy.revision(),
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex Primary",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint publish");
    let published = publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            ProviderCredentialDraft::new(
                "Primary Key",
                CredentialKind::ApiKey,
                proxy_id,
                MaxConcurrency::new(2).expect("max concurrency"),
                true,
            )
            .expect("credential draft"),
            ProviderApiKeySecret::new("sk-balancing-contract".to_owned()),
        )
        .await
        .expect("credential publish");
    let permit = published
        .credential_runtime(credential_id.into())
        .expect("credential runtime")
        .try_acquire()
        .expect("capacity permit");
    let app = test_router(
        &directory,
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        publisher,
    );

    let (status, headers, body) = request(app, "/api/admin/balancing").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(CACHE_CONTROL).expect("no-store"), "no-store");
    assert_eq!(body["config_revision"], 4);
    assert_eq!(body["queue"]["waiting"], 0);
    assert_eq!(body["queue"]["max_waiting"], 128);
    assert_eq!(body["queue"]["timeout_secs"], 30);
    assert_eq!(body["queue"]["on_saturated"], "wait");
    assert_eq!(body["auxiliary"]["max_global"], 32);
    assert_eq!(body["totals"]["in_flight"], 1);
    assert_eq!(body["totals"]["enabled_credential_count"], 0);
    assert_eq!(body["totals"]["max_concurrency"], 0);
    let credential = &body["credentials"][0];
    assert_eq!(credential["credential_id"], credential_id.to_string());
    assert_eq!(credential["label"], "Primary Key");
    assert_eq!(credential["provider_kind"], "codex");
    assert_eq!(credential["endpoint_name"], "Codex Primary");
    assert_eq!(credential["proxy_name"], "Disabled Proxy");
    assert_eq!(credential["proxy_enabled"], false);
    assert_eq!(credential["in_flight"], 1);
    assert_eq!(credential["max_concurrency"], 2);
    assert_eq!(credential["counters"]["selected_generation"], 0);
    assert!(credential["models"].as_array().is_some_and(Vec::is_empty));
    drop(permit);
}

fn test_router(
    directory: &tempfile::TempDir,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: Arc<ConfigPublisher>,
) -> Router {
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    build_router(
        AppState::new(snapshots, runtime, publisher, public_requests),
        web_root,
    )
}

async fn request(app: Router, uri: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 41000))))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).expect("json"),
    )
}
