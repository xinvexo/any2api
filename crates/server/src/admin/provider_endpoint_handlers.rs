use std::str::FromStr;

use any2api_domain::ProviderEndpointId;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    provider_endpoint_dto::{ProviderEndpointCollectionResponse, ProviderEndpointWriteRequest},
    revision::ExpectedRevisionQuery,
};

pub(crate) async fn list(
    State(state): State<AppState>,
) -> Json<ProviderEndpointCollectionResponse> {
    let snapshot = state.snapshots().load();
    Json(ProviderEndpointCollectionResponse::from_snapshot(
        &snapshot,
        state.publisher().configuration_capabilities(),
    ))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    payload: Result<Json<ProviderEndpointWriteRequest>, JsonRejection>,
) -> Result<Json<ProviderEndpointCollectionResponse>, AdminApiError> {
    let request = parse_json(payload)?;
    let (expected, draft) = request.into_create_domain()?;
    let snapshot = state
        .publisher()
        .create_provider_endpoint(expected, ProviderEndpointId::new(), draft)
        .await?;
    Ok(Json(ProviderEndpointCollectionResponse::from_snapshot(
        &snapshot,
        state.publisher().configuration_capabilities(),
    )))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<ProviderEndpointWriteRequest>, JsonRejection>,
) -> Result<Json<ProviderEndpointCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let request = parse_json(payload)?;
    let (expected, expected_config_version, draft) = request.into_update_domain()?;
    let snapshot = state
        .publisher()
        .update_provider_endpoint(expected, id, expected_config_version, draft)
        .await?;
    Ok(Json(ProviderEndpointCollectionResponse::from_snapshot(
        &snapshot,
        state.publisher().configuration_capabilities(),
    )))
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    query: Result<Query<ExpectedRevisionQuery>, QueryRejection>,
) -> Result<Json<ProviderEndpointCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let expected = query
        .map_err(|_| AdminApiError::invalid_request("expected_revision query is required"))?
        .0
        .revision()?;
    let snapshot = state
        .publisher()
        .delete_provider_endpoint(expected, id)
        .await?;
    Ok(Json(ProviderEndpointCollectionResponse::from_snapshot(
        &snapshot,
        state.publisher().configuration_capabilities(),
    )))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn parse_id(value: &str) -> Result<ProviderEndpointId, AdminApiError> {
    ProviderEndpointId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("provider endpoint id is invalid"))
}
