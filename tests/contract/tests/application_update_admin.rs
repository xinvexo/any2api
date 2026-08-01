use std::{fs, net::SocketAddr, sync::Arc};

use any2api_contract_tests::build_public_request_components;
use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_updater::api::{
    ApplicationAbout, ApplicationUpdateService, UpdateCheck, UpdateError, UpdateErrorKind,
    UpdateStatus,
};
use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    extract::ConnectInfo,
    http::{Method, Request, StatusCode, header::CACHE_CONTROL},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tempfile::tempdir;
use tower::ServiceExt;

struct SuccessfulUpdates;

#[async_trait]
impl ApplicationUpdateService for SuccessfulUpdates {
    fn about(&self) -> ApplicationAbout {
        ApplicationAbout {
            current_version: "1.0.0".to_owned(),
            repository_url: "https://github.com/xinvexo/any2api".to_owned(),
        }
    }

    async fn check(&self) -> Result<UpdateCheck, UpdateError> {
        Ok(UpdateCheck {
            current_version: "1.0.0".to_owned(),
            latest_version: "1.1.0".to_owned(),
            update_available: true,
            release_url: "https://github.com/xinvexo/any2api/releases/tag/v1.1.0".to_owned(),
            published_at: Some("2026-07-29T00:00:00Z".to_owned()),
        })
    }

    fn start_install(&self) -> Result<UpdateStatus, UpdateError> {
        Ok(UpdateStatus::Checking)
    }

    fn install_status(&self) -> UpdateStatus {
        UpdateStatus::Downloading {
            target_version: "1.1.0".to_owned(),
            downloaded_bytes: 512,
            total_bytes: 1024,
        }
    }
}

struct FailingUpdates(UpdateErrorKind);

#[async_trait]
impl ApplicationUpdateService for FailingUpdates {
    fn about(&self) -> ApplicationAbout {
        SuccessfulUpdates.about()
    }

    async fn check(&self) -> Result<UpdateCheck, UpdateError> {
        Err(UpdateError::new(self.0, "test failure"))
    }

    fn start_install(&self) -> Result<UpdateStatus, UpdateError> {
        Err(UpdateError::new(self.0, "test failure"))
    }

    fn install_status(&self) -> UpdateStatus {
        UpdateStatus::Failed {
            target_version: Some("1.1.0".to_owned()),
            kind: self.0,
        }
    }
}

#[tokio::test]
async fn update_admin_exposes_about_check_and_install_contracts() {
    let (_directory, app) = test_app(Some(Arc::new(SuccessfulUpdates))).await;
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));

    let (status, headers, about) =
        request(app.clone(), Method::GET, "/api/admin/about", loopback).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(CACHE_CONTROL).expect("no-store"), "no-store");
    assert_eq!(about["current_version"], "1.0.0");
    assert_eq!(
        about["repository_url"],
        "https://github.com/xinvexo/any2api"
    );
    assert!(about.get("install_supported").is_none());
    assert!(about.get("install_support_reason").is_none());

    let (status, _, check) = request(
        app.clone(),
        Method::POST,
        "/api/admin/update/check",
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(check["current_version"], "1.0.0");
    assert_eq!(check["latest_version"], "1.1.0");
    assert_eq!(check["update_available"], true);
    assert_eq!(check["published_at"], "2026-07-29T00:00:00Z");

    let (status, _, accepted) = request(
        app.clone(),
        Method::POST,
        "/api/admin/update/install",
        loopback,
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(accepted["phase"], "checking");
    assert!(accepted["target_version"].is_null());

    let (status, _, progress) =
        request(app, Method::GET, "/api/admin/update/status", loopback).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(progress["phase"], "downloading");
    assert_eq!(progress["target_version"], "1.1.0");
    assert_eq!(progress["downloaded_bytes"], 512);
    assert_eq!(progress["total_bytes"], 1024);
    assert!(progress["failure_code"].is_null());
}

#[tokio::test]
async fn update_admin_remains_protected_and_reports_missing_service() {
    let remote = SocketAddr::from(([203, 0, 113, 5], 41000));
    let (_directory, app) = test_app(Some(Arc::new(SuccessfulUpdates))).await;
    let (status, _, body) = request(app, Method::GET, "/api/admin/about", remote).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "admin_loopback_only");

    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let (_directory, app) = test_app(None).await;
    let (status, _, body) = request(app, Method::GET, "/api/admin/about", loopback).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "update_unavailable");
}

#[tokio::test]
async fn update_errors_map_to_stable_admin_codes() {
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    for (kind, expected_status, expected_code) in [
        (
            UpdateErrorKind::Unsupported,
            StatusCode::CONFLICT,
            "update_unsupported",
        ),
        (
            UpdateErrorKind::NoUpdate,
            StatusCode::CONFLICT,
            "update_not_available",
        ),
        (
            UpdateErrorKind::InProgress,
            StatusCode::CONFLICT,
            "update_in_progress",
        ),
        (
            UpdateErrorKind::ShuttingDown,
            StatusCode::CONFLICT,
            "update_shutting_down",
        ),
        (
            UpdateErrorKind::CheckFailed,
            StatusCode::BAD_GATEWAY,
            "update_check_failed",
        ),
        (
            UpdateErrorKind::InvalidRelease,
            StatusCode::BAD_GATEWAY,
            "update_check_failed",
        ),
        (
            UpdateErrorKind::DownloadFailed,
            StatusCode::BAD_GATEWAY,
            "update_download_failed",
        ),
        (
            UpdateErrorKind::VerificationFailed,
            StatusCode::BAD_GATEWAY,
            "update_verification_failed",
        ),
        (
            UpdateErrorKind::InstallFailed,
            StatusCode::INTERNAL_SERVER_ERROR,
            "update_install_failed",
        ),
    ] {
        let (_directory, app) = test_app(Some(Arc::new(FailingUpdates(kind)))).await;
        let (status, _, body) = request(
            app.clone(),
            Method::POST,
            "/api/admin/update/install",
            loopback,
        )
        .await;
        assert_eq!(status, expected_status, "{kind:?}");
        assert_eq!(body["error"]["code"], expected_code, "{kind:?}");

        let (status, _, progress) =
            request(app, Method::GET, "/api/admin/update/status", loopback).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(progress["phase"], "failed");
        assert_eq!(progress["failure_code"], expected_code, "{kind:?}");
    }
}

async fn test_app(
    updates: Option<Arc<dyn ApplicationUpdateService>>,
) -> (tempfile::TempDir, Router) {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new());
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            any2api_contract_tests::build_provider_registry().as_ref(),
        )
        .expect("initial snapshot"),
    ));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            any2api_contract_tests::build_configuration_capabilities(),
        )
        .expect("publisher"),
    );
    let web_root = directory.path().join("web");
    fs::create_dir(&web_root).expect("web directory");
    fs::write(web_root.join("index.html"), "<main>any2api</main>").expect("web index");
    let mut state = AppState::new(
        snapshots,
        runtime,
        publisher,
        build_public_request_components()
            .expect("components")
            .service(),
    );
    if let Some(updates) = updates {
        state = state.with_application_updates(updates);
    }
    (directory, build_router(state, web_root))
}

async fn request(
    app: Router,
    method: Method,
    uri: &str,
    remote: SocketAddr,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .extension(ConnectInfo(remote))
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
