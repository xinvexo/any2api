use any2api_updater::api::ApplicationUpdateService;
use axum::{Json, extract::State, http::StatusCode};

use crate::{admin::AdminApiError, state::AppState};

use super::{
    dto::{AboutResponse, UpdateCheckResponse, UpdateStatusResponse},
    error::map_error,
};

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
