use std::{fs, sync::Arc};

use any2api_runtime::api::{PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::SqliteStore;
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn sqlite_bootstrap_and_health_route_share_the_loaded_revision() {
    let directory = tempdir().expect("temporary directory");
    let storage = SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
        .await
        .expect("sqlite bootstrap");
    let revision = storage
        .load_config_revision()
        .await
        .expect("configuration revision");
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api shell</main>").expect("web index");
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(revision)));
    let runtime = Arc::new(RuntimeRegistry::new());
    let app = build_router(AppState::new(snapshots, runtime), web_root);
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

    let missing_api = app
        .oneshot(
            Request::builder()
                .uri("/api/missing")
                .body(Body::empty())
                .expect("missing api request"),
        )
        .await
        .expect("missing api response");
    assert_eq!(missing_api.status(), 404);
}
