use any2api_updater::api::{ApplicationUpdateService, UpdateError, UpdateErrorKind};
use axum::{Json, extract::State, http::StatusCode};

use crate::{admin::AdminApiError, state::AppState};

use super::dto::{AboutResponse, UpdateCheckResponse, UpdateInstallResponse};

pub(crate) async fn about(
    State(state): State<AppState>,
) -> Result<Json<AboutResponse>, AdminApiError> {
    let updates = service(&state)?;
    Ok(Json(updates.about().into()))
}

pub(crate) async fn check(
    State(state): State<AppState>,
) -> Result<Json<UpdateCheckResponse>, AdminApiError> {
    let result = service(&state)?.check().await.map_err(map_error)?;
    Ok(Json(result.into()))
}

pub(crate) async fn install(
    State(state): State<AppState>,
) -> Result<Json<UpdateInstallResponse>, AdminApiError> {
    let result = service(&state)?.install().await.map_err(map_error)?;
    Ok(Json(result.into()))
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

fn map_error(error: UpdateError) -> AdminApiError {
    let kind = error.kind();
    tracing::warn!(?kind, %error, "application update operation failed");
    match kind {
        UpdateErrorKind::Unsupported => AdminApiError::new(
            StatusCode::CONFLICT,
            "update_unsupported",
            "this runtime environment does not support automatic updates",
        ),
        UpdateErrorKind::NoUpdate => AdminApiError::new(
            StatusCode::CONFLICT,
            "update_not_available",
            "the current version is already up to date",
        ),
        UpdateErrorKind::InProgress => AdminApiError::new(
            StatusCode::CONFLICT,
            "update_in_progress",
            "an application update is already in progress",
        ),
        UpdateErrorKind::CheckFailed | UpdateErrorKind::InvalidRelease => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "update_check_failed",
            "the latest official release could not be verified",
        ),
        UpdateErrorKind::DownloadFailed => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "update_download_failed",
            "the official release could not be downloaded",
        ),
        UpdateErrorKind::VerificationFailed => AdminApiError::new(
            StatusCode::BAD_GATEWAY,
            "update_verification_failed",
            "the downloaded release did not pass verification",
        ),
        UpdateErrorKind::InstallFailed => AdminApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "update_install_failed",
            "the verified release could not replace the current executable",
        ),
    }
}
