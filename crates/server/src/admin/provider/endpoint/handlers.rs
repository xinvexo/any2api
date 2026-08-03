use std::str::FromStr;

use any2api_domain::ProviderEndpointId;
use axum::{
    Json,
    extract::{Path, State},
};

use crate::state::AppState;

use super::{
    dto::{ProviderEndpointCollectionResponse, ProviderEndpointWriteRequest},
    error::AdminApiError,
    request_json::AdminJson,
    revision::RequiredRevisionQuery,
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
    AdminJson(request): AdminJson<ProviderEndpointWriteRequest>,
) -> Result<Json<ProviderEndpointCollectionResponse>, AdminApiError> {
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
    AdminJson(request): AdminJson<ProviderEndpointWriteRequest>,
) -> Result<Json<ProviderEndpointCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
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
    RequiredRevisionQuery(expected): RequiredRevisionQuery,
) -> Result<Json<ProviderEndpointCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let snapshot = state
        .publisher()
        .delete_provider_endpoint(expected, id)
        .await?;
    Ok(Json(ProviderEndpointCollectionResponse::from_snapshot(
        &snapshot,
        state.publisher().configuration_capabilities(),
    )))
}

fn parse_id(value: &str) -> Result<ProviderEndpointId, AdminApiError> {
    ProviderEndpointId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("provider endpoint id is invalid"))
}
