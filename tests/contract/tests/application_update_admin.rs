use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use any2api_contract_tests::TestApplication;
use any2api_updater::api::{
    ApplicationAbout, ApplicationUpdateService, RestartKind, RestartRequestStatus,
    RestartRequester, UpdateCheck, UpdateError, UpdateErrorKind, UpdateStatus,
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

struct TestRestartRequester {
    status: RestartRequestStatus,
    calls: AtomicUsize,
}

impl TestRestartRequester {
    fn new(status: RestartRequestStatus) -> Self {
        Self {
            status,
            calls: AtomicUsize::new(0),
        }
    }
}

impl RestartRequester for TestRestartRequester {
    fn request_restart(&self, kind: RestartKind) -> RestartRequestStatus {
        assert_eq!(kind, RestartKind::Manual);
        self.calls.fetch_add(1, Ordering::AcqRel);
        self.status
    }
}

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
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "admin_session_required");

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

#[tokio::test]
async fn manual_restart_is_protected_accepted_once_and_reports_stable_rejections() {
    let loopback = SocketAddr::from(([127, 0, 0, 1], 41000));
    let remote = SocketAddr::from(([203, 0, 113, 5], 41000));

    let accepted = Arc::new(TestRestartRequester::new(RestartRequestStatus::Accepted));
    let (_directory, app) = test_app_with_restart(None, Some(accepted.clone())).await;
    let (status, _, body) = request(app.clone(), Method::POST, "/api/admin/restart", remote).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "admin_session_required");
    assert_eq!(accepted.calls.load(Ordering::Acquire), 0);

    let (status, _, body) = request(app, Method::POST, "/api/admin/restart", loopback).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body, serde_json::json!({ "status": "restarting" }));
    assert_eq!(accepted.calls.load(Ordering::Acquire), 1);

    let unsupported = Arc::new(TestRestartRequester::new(RestartRequestStatus::Unsupported));
    let (_directory, app) = test_app_with_restart(None, Some(unsupported)).await;
    let (status, _, body) = request(app, Method::POST, "/api/admin/restart", loopback).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "restart_unsupported");

    let (_directory, app) = test_app_with_restart(None, None).await;
    let (status, _, body) = request(app, Method::POST, "/api/admin/restart", loopback).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "restart_unavailable");

    let blocked = Arc::new(TestRestartRequester::new(RestartRequestStatus::Accepted));
    let (_directory, app) =
        test_app_with_restart(Some(Arc::new(SuccessfulUpdates)), Some(blocked.clone())).await;
    let (status, _, body) = request(app, Method::POST, "/api/admin/restart", loopback).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "restart_update_in_progress");
    assert_eq!(blocked.calls.load(Ordering::Acquire), 0);
}

async fn test_app(
    updates: Option<Arc<dyn ApplicationUpdateService>>,
) -> (tempfile::TempDir, Router) {
    test_app_with_restart(updates, None).await
}

async fn test_app_with_restart(
    updates: Option<Arc<dyn ApplicationUpdateService>>,
    restart: Option<Arc<dyn RestartRequester>>,
) -> (tempfile::TempDir, Router) {
    let fixture = TestApplication::new().await;
    let mut state = fixture.state();
    if let Some(updates) = updates {
        state = state.with_application_updates(updates);
    }
    if let Some(restart) = restart {
        state = state.with_restart_requester(restart);
    }
    let (directory, app, _storage) = fixture.into_router_with_state(state);
    (directory, app)
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
