use any2api_domain::SettingKey;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    revision::ExpectedRevisionQuery,
    settings_dto::{SettingWriteRequest, SettingsResponse},
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<SettingsResponse> {
    Json(SettingsResponse::from_snapshot(&state.snapshots().load()))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(key): Path<String>,
    payload: Result<Json<SettingWriteRequest>, JsonRejection>,
) -> Result<Json<SettingsResponse>, AdminApiError> {
    let key = parse_key(&key)?;
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
    let key = parse_key(&key)?;
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

fn parse_key(value: &str) -> Result<SettingKey, AdminApiError> {
    SettingKey::parse(value).ok_or_else(AdminApiError::setting_not_found)
}
