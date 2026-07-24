use std::{fs, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn sqlite_bootstrap_and_health_route_share_the_loaded_revision() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("sqlite bootstrap"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::create_dir(web_root.join("assets")).expect("asset directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    fs::write(web_root.join("assets/app.js"), "console.log('asset')").expect("web asset");
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
        any2api_contract_tests::build_provider_registry().as_ref(),
    )));
    let publisher = Arc::new(
        ConfigPublisher::new(
            storage,
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            any2api_contract_tests::build_configuration_capabilities(),
        )
        .expect("configuration publisher"),
    );
    let public_requests = build_public_request_components()
        .expect("public request components")
        .service();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, public_requests),
        web_root,
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/health")
                .body(Body::empty())
                .expect("health request"),
        )
        .await
        .expect("health response");

    assert_eq!(response.status(), 200);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("health body")
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).expect("health json");

    assert_eq!(value["status"], "ok");
    assert_eq!(value["config_revision"], 1);
    assert_eq!(value["scheduler_epoch"], 0);
    assert_eq!(value["shutdown_phase"], "running");
    assert_eq!(value["active_requests"], 0);
    assert_eq!(value["background_tasks"], 0);

    let deep_link = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/settings")
                .body(Body::empty())
                .expect("deep link request"),
        )
        .await
        .expect("deep link response");
    assert_eq!(deep_link.status(), 200);
    let deep_link_body = deep_link
        .into_body()
        .collect()
        .await
        .expect("deep link body")
        .to_bytes();
    assert!(
        deep_link_body
            .windows(13)
            .any(|part| part == b"any2api shell")
    );

    let log_deep_link = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/logs/11111111-1111-4111-8111-111111111111")
                .body(Body::empty())
                .expect("request log deep link request"),
        )
        .await
        .expect("request log deep link response");
    assert_eq!(log_deep_link.status(), 200);

    let asset = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/app.js")
                .body(Body::empty())
                .expect("asset request"),
        )
        .await
        .expect("asset response");
    assert_eq!(asset.status(), 200);

    let missing_asset = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/missing.js")
                .body(Body::empty())
                .expect("missing asset request"),
        )
        .await
        .expect("missing asset response");
    assert_eq!(missing_asset.status(), 404);

    let rejected_write = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/settings")
                .body(Body::empty())
                .expect("static write request"),
        )
        .await
        .expect("static write response");
    assert_eq!(rejected_write.status(), 405);

    for uri in ["/api", "/api/", "/api/missing"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("missing api request"),
            )
            .await
            .expect("missing api response");
        assert_eq!(response.status(), 404, "unexpected status for {uri}");
    }

    for uri in ["/v1", "/v1/"] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("public api root request"),
            )
            .await
            .expect("public api root response");
        assert_eq!(response.status(), 401, "unexpected status for {uri}");
    }
}
