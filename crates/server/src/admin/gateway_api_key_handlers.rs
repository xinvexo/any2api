use std::str::FromStr;

use any2api_domain::GatewayApiKeyId;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    response::Response,
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    gateway_api_key_dto::{
        GatewayApiKeyCollectionResponse, GatewayApiKeyCreateRequest, GatewayApiKeyRevokeRequest,
        GatewayApiKeyRotateRequest, GatewayApiKeySecretResponse, GatewayApiKeyUpdateRequest,
    },
    no_store,
};

pub(crate) async fn list(State(state): State<AppState>) -> Response {
    response(&state, &state.snapshots().load())
}

pub(crate) async fn create(
    State(state): State<AppState>,
    payload: Result<Json<GatewayApiKeyCreateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (expected, draft) = parse_json(payload)?.into_domain()?;
    let published = state
        .publisher()
        .create_gateway_api_key(expected, GatewayApiKeyId::new(), draft)
        .await?;
    Ok(no_store::json(GatewayApiKeySecretResponse::from_publish(
        &published,
        state.request_telemetry(),
    )))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<GatewayApiKeyUpdateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version, draft) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .update_gateway_api_key(expected, id, expected_config_version, draft)
        .await?;
    Ok(response(&state, &snapshot))
}

pub(crate) async fn rotate(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<GatewayApiKeyRotateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version, expected_token_version) =
        parse_json(payload)?.into_domain()?;
    let published = state
        .publisher()
        .rotate_gateway_api_key(
            expected,
            id,
            expected_config_version,
            expected_token_version,
        )
        .await?;
    Ok(no_store::json(GatewayApiKeySecretResponse::from_publish(
        &published,
        state.request_telemetry(),
    )))
}

pub(crate) async fn revoke(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<GatewayApiKeyRevokeRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .revoke_gateway_api_key(expected, id, expected_config_version)
        .await?;
    Ok(response(&state, &snapshot))
}

fn response(state: &AppState, snapshot: &any2api_runtime::api::PublishedSnapshot) -> Response {
    no_store::json(GatewayApiKeyCollectionResponse::from_snapshot(
        snapshot,
        state.request_telemetry(),
    ))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn parse_id(value: &str) -> Result<GatewayApiKeyId, AdminApiError> {
    GatewayApiKeyId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("gateway API Key id is invalid"))
}
