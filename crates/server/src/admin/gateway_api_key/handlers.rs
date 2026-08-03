use std::str::FromStr;

use any2api_domain::GatewayApiKeyId;
use axum::{
    Json,
    extract::{Path, State},
};

use crate::state::AppState;

use super::{
    dto::{
        GatewayApiKeyCollectionResponse, GatewayApiKeyCreateRequest, GatewayApiKeyDeleteRequest,
        GatewayApiKeyRotateRequest, GatewayApiKeyUpdateRequest,
    },
    error::AdminApiError,
    request_json::AdminJson,
    revision::RequiredVersionedQuery,
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<GatewayApiKeyCollectionResponse> {
    response_for_snapshot(&state, &state.snapshots().load()).await
}

pub(crate) async fn create(
    State(state): State<AppState>,
    AdminJson(payload): AdminJson<GatewayApiKeyCreateRequest>,
) -> Result<Json<GatewayApiKeyCollectionResponse>, AdminApiError> {
    let (expected, draft) = payload.into_domain()?;
    let snapshot = state
        .publisher()
        .create_gateway_api_key(expected, GatewayApiKeyId::new(), draft)
        .await?;
    Ok(response_for_snapshot(&state, &snapshot).await)
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(payload): AdminJson<GatewayApiKeyUpdateRequest>,
) -> Result<Json<GatewayApiKeyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version, draft) = payload.into_domain()?;
    let snapshot = state
        .publisher()
        .update_gateway_api_key(expected, id, expected_config_version, draft)
        .await?;
    Ok(response_for_snapshot(&state, &snapshot).await)
}

pub(crate) async fn rotate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(payload): AdminJson<GatewayApiKeyRotateRequest>,
) -> Result<Json<GatewayApiKeyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version, expected_token_version) = payload.into_domain()?;
    let snapshot = state
        .publisher()
        .rotate_gateway_api_key(
            expected,
            id,
            expected_config_version,
            expected_token_version,
        )
        .await?;
    Ok(response_for_snapshot(&state, &snapshot).await)
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RequiredVersionedQuery(query): RequiredVersionedQuery<GatewayApiKeyDeleteRequest>,
) -> Result<Json<GatewayApiKeyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version) = query.into_domain()?;
    let snapshot = state
        .publisher()
        .delete_gateway_api_key(expected, id, expected_config_version)
        .await?;
    Ok(response_for_snapshot(&state, &snapshot).await)
}

async fn response_for_snapshot(
    state: &AppState,
    snapshot: &any2api_runtime::api::PublishedSnapshot,
) -> Json<GatewayApiKeyCollectionResponse> {
    let usage = usage(state).await;
    Json(GatewayApiKeyCollectionResponse::from_snapshot(
        snapshot,
        state.request_telemetry(),
        &usage,
    ))
}

async fn usage(state: &AppState) -> Vec<any2api_runtime::api::GatewayApiKeyUsageSummary> {
    match state.request_telemetry().gateway_key_usage().await {
        Ok(usage) => usage,
        Err(error) => {
            tracing::warn!(%error, "gateway API Key usage statistics unavailable");
            Vec::new()
        }
    }
}

fn parse_id(value: &str) -> Result<GatewayApiKeyId, AdminApiError> {
    GatewayApiKeyId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("gateway API Key id is invalid"))
}
