use std::str::FromStr;

use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
    response::Response,
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    no_store,
    oauth_dto::{
        OAuthAccountCollectionResponse, OAuthAccountDeleteQuery, OAuthAccountModelsRequest,
        OAuthAccountUpdateRequest, OAuthExchangeRequest, OAuthExchangeResponse, OAuthStartRequest,
        OAuthStartResponse,
    },
    oauth_error,
    oauth_quota_dto::{OAuthQuotaResetResponse, OAuthQuotaResponse},
    oauth_quota_error,
};

pub(super) async fn list(State(state): State<AppState>) -> Result<Response, AdminApiError> {
    Ok(accounts_response(&state.snapshots().load()))
}

pub(super) async fn start(
    State(state): State<AppState>,
    payload: Result<Json<OAuthStartRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let request = parse_json(payload)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .start(request.provider())
        .await
        .map_err(oauth_error::map)?;
    Ok(no_store::json(OAuthStartResponse::from(result)))
}

pub(super) async fn exchange(
    State(state): State<AppState>,
    payload: Result<Json<OAuthExchangeRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (session_id, callback_url) = parse_json(payload)?.into_parts();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .exchange(&session_id, &callback_url)
        .await
        .map_err(oauth_error::map)?;
    Ok(no_store::json(OAuthExchangeResponse::from(result)))
}

pub(super) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<OAuthAccountUpdateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let (expected, expected_config_version, draft) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .update_oauth_account(expected, id, expected_config_version, draft)
        .await?;
    Ok(accounts_response(&snapshot))
}

pub(super) async fn set_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<OAuthAccountModelsRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let (expected, expected_config_version, models) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .set_oauth_account_models(expected, id, expected_config_version, models)
        .await?;
    Ok(accounts_response(&snapshot))
}

pub(super) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    query: Result<Query<OAuthAccountDeleteQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let (expected, expected_config_version) = query
        .map_err(|_| {
            AdminApiError::invalid_request(
                "expected_revision and expected_config_version queries are required",
            )
        })?
        .0
        .into_domain()?;
    let snapshot = state
        .publisher()
        .delete_oauth_account(expected, id, expected_config_version)
        .await?;
    Ok(accounts_response(&snapshot))
}

pub(super) async fn quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .query_quota(id)
        .await
        .map_err(oauth_quota_error::map)?;
    Ok(no_store::json(OAuthQuotaResponse::from(result)))
}

pub(super) async fn reset_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let id = parse_account_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .reset_quota(id)
        .await
        .map_err(oauth_quota_error::map)?;
    Ok(no_store::json(OAuthQuotaResetResponse::from(result)))
}

fn accounts_response(snapshot: &any2api_runtime::api::PublishedSnapshot) -> Response {
    no_store::json(OAuthAccountCollectionResponse::from_snapshot(snapshot))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}

fn parse_account_id(value: &str) -> Result<any2api_domain::OAuthAccountId, AdminApiError> {
    any2api_domain::OAuthAccountId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("OAuth account id is invalid"))
}
