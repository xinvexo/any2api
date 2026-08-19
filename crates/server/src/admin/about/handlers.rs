use any2api_updater::api::{
    ApplicationUpdateService, RestartKind, RestartRequestStatus, RestartRequester,
};
use axum::{Json, extract::State, http::StatusCode};

use crate::{admin::AdminApiError, state::AppState};

use super::{
    dto::{AboutResponse, RestartResponse, UpdateCheckResponse, UpdateStatusResponse},
    error::map_error,
};

pub(crate) async fn about(
    State(state): State<AppState>,
) -> Result<Json<AboutResponse>, AdminApiError> {
    let updates = service(&state)?;
    Ok(Json(updates.about().into()))
}

pub(crate) async fn restart(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<RestartResponse>), AdminApiError> {
    let response = request_manual_restart(state.restart_requester(), state.application_updates())?;
    Ok((StatusCode::ACCEPTED, Json(response)))
}

pub(crate) async fn check(
    State(state): State<AppState>,
) -> Result<Json<UpdateCheckResponse>, AdminApiError> {
    let result = service(&state)?.check().await.map_err(map_error)?;
    Ok(Json(result.into()))
}

pub(crate) async fn install(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<UpdateStatusResponse>), AdminApiError> {
    let result = service(&state)?.start_install().map_err(map_error)?;
    Ok((StatusCode::ACCEPTED, Json(result.into())))
}

pub(crate) async fn status(
    State(state): State<AppState>,
) -> Result<Json<UpdateStatusResponse>, AdminApiError> {
    Ok(Json(service(&state)?.install_status().into()))
}

fn service(state: &AppState) -> Result<&dyn ApplicationUpdateService, AdminApiError> {
    state.application_updates().ok_or_else(|| {
        AdminApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "update_unavailable",
            "application updates are unavailable",
        )
    })
}

fn request_manual_restart(
    restart: Option<&dyn RestartRequester>,
    updates: Option<&dyn ApplicationUpdateService>,
) -> Result<RestartResponse, AdminApiError> {
    let restart = restart.ok_or_else(|| {
        AdminApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "restart_unavailable",
            "application restart is unavailable",
        )
    })?;
    if updates.is_some_and(|updates| updates.install_status().is_active()) {
        return Err(AdminApiError::new(
            StatusCode::CONFLICT,
            "restart_update_in_progress",
            "application restart is unavailable while an update is in progress",
        ));
    }
    match restart.request_restart(RestartKind::Manual) {
        RestartRequestStatus::Accepted | RestartRequestStatus::AlreadyRequested => {
            Ok(RestartResponse::restarting())
        }
        RestartRequestStatus::Unsupported => Err(AdminApiError::new(
            StatusCode::CONFLICT,
            "restart_unsupported",
            "this runtime environment does not support application restart",
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use any2api_updater::api::{
        ApplicationAbout, RestartKind, RestartRequestStatus, RestartRequester, UpdateCheck,
        UpdateError, UpdateStatus,
    };
    use async_trait::async_trait;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    use super::{ApplicationUpdateService, request_manual_restart};

    struct StubRestart {
        status: RestartRequestStatus,
        calls: AtomicUsize,
    }

    impl StubRestart {
        fn new(status: RestartRequestStatus) -> Self {
            Self {
                status,
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl RestartRequester for StubRestart {
        fn request_restart(&self, kind: RestartKind) -> RestartRequestStatus {
            assert_eq!(kind, RestartKind::Manual);
            self.calls.fetch_add(1, Ordering::AcqRel);
            self.status
        }
    }

    struct StubUpdates(UpdateStatus);

    #[async_trait]
    impl ApplicationUpdateService for StubUpdates {
        fn about(&self) -> ApplicationAbout {
            unreachable!("about is not used by restart tests")
        }

        async fn check(&self) -> Result<UpdateCheck, UpdateError> {
            unreachable!("check is not used by restart tests")
        }

        fn start_install(&self) -> Result<UpdateStatus, UpdateError> {
            unreachable!("install is not used by restart tests")
        }

        fn install_status(&self) -> UpdateStatus {
            self.0.clone()
        }
    }

    #[test]
    fn accepted_and_duplicate_restart_requests_return_the_same_contract() {
        for status in [
            RestartRequestStatus::Accepted,
            RestartRequestStatus::AlreadyRequested,
        ] {
            let restart = StubRestart::new(status);
            let response = request_manual_restart(Some(&restart), None).expect("restart accepted");
            assert_eq!(
                serde_json::to_value(response).expect("restart JSON"),
                serde_json::json!({ "status": "restarting" })
            );
            assert_eq!(restart.calls.load(Ordering::Acquire), 1);
        }
    }

    #[tokio::test]
    async fn active_update_rejects_restart_before_signalling() {
        let restart = StubRestart::new(RestartRequestStatus::Accepted);
        let updates = StubUpdates(UpdateStatus::Downloading {
            target_version: "1.2.3".to_owned(),
            downloaded_bytes: 4,
            total_bytes: 10,
        });

        let error = request_manual_restart(Some(&restart), Some(&updates))
            .expect_err("active update must reject restart");
        assert_error(error, 409, "restart_update_in_progress").await;
        assert_eq!(restart.calls.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn unsupported_and_missing_restart_services_have_stable_errors() {
        let unsupported = StubRestart::new(RestartRequestStatus::Unsupported);
        let error = request_manual_restart(Some(&unsupported), None)
            .expect_err("unsupported restart must fail");
        assert_error(error, 409, "restart_unsupported").await;

        let error = request_manual_restart(None, None).expect_err("missing restart service");
        assert_error(error, 503, "restart_unavailable").await;
    }

    async fn assert_error(error: crate::admin::AdminApiError, status: u16, code: &str) {
        let response = error.into_response();
        assert_eq!(response.status().as_u16(), status);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("error body")
            .to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(body["error"]["code"], code);
    }
}
