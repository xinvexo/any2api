use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
};

use crate::state::AppState;

use super::{
    dto::{SettingBatchWriteRequest, SettingWriteRequest, SettingsResponse, parse_setting_key},
    error::AdminApiError,
    revision::ExpectedRevisionQuery,
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<SettingsResponse> {
    Json(SettingsResponse::from_snapshot(&state.snapshots().load()))
}

pub(crate) async fn update_batch(
    State(state): State<AppState>,
    payload: Result<Json<SettingBatchWriteRequest>, JsonRejection>,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let request = payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))?;
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
    payload: Result<Json<SettingWriteRequest>, JsonRejection>,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let key = parse_setting_key(&key)?;
    let request = payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))?;
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
    query: Result<Query<ExpectedRevisionQuery>, QueryRejection>,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let key = parse_setting_key(&key)?;
    let expected = query
        .map_err(|_| AdminApiError::invalid_request("expected_revision query is required"))?
        .0
        .revision()?;
    let snapshot = state
        .publisher()
        .reset_setting_override(expected, key)
        .await?;
    Ok(Json(SettingsResponse::from_snapshot(&snapshot)))
}
