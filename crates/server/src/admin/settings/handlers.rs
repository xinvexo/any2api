use axum::{
    Json,
    extract::{Path, State},
};

use crate::state::AppState;

use super::{
    dto::{SettingBatchWriteRequest, SettingWriteRequest, SettingsResponse, parse_setting_key},
    error::AdminApiError,
    request_json::AdminJson,
    revision::RequiredRevisionQuery,
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<SettingsResponse> {
    Json(SettingsResponse::from_snapshot(&state.snapshots().load()))
}

pub(crate) async fn update_batch(
    State(state): State<AppState>,
    AdminJson(request): AdminJson<SettingBatchWriteRequest>,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let (expected, changes) = request.into_domain()?;
    let snapshot = state
        .publisher()
        .apply_setting_changes(expected, changes)
        .await?;
    Ok(Json(SettingsResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(key): Path<String>,
    AdminJson(request): AdminJson<SettingWriteRequest>,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let key = parse_setting_key(&key)?;
    let (expected, value) = request.into_domain(key)?;
    let snapshot = state
        .publisher()
        .set_setting_override(expected, key, value)
        .await?;
    Ok(Json(SettingsResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn reset(
    State(state): State<AppState>,
    Path(key): Path<String>,
    RequiredRevisionQuery(expected): RequiredRevisionQuery,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let key = parse_setting_key(&key)?;
    let snapshot = state
        .publisher()
        .reset_setting_override(expected, key)
        .await?;
    Ok(Json(SettingsResponse::from_snapshot(&snapshot)))
}
