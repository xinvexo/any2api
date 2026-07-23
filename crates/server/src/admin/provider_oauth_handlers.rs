use std::str::FromStr;

use any2api_domain::ProviderEndpointId;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    response::Response,
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    no_store,
    provider_oauth_dto::{
        ProviderOAuthExchangeBody, ProviderOAuthExchangeResponse, ProviderOAuthStartBody,
        ProviderOAuthStartResponse,
    },
};

pub(crate) async fn start(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    payload: Result<Json<ProviderOAuthStartBody>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let endpoint_id = parse_endpoint_id(&endpoint_id)?;
    let request = parse_json(payload)?.into_domain()?;
    let service = state
        .provider_oauth()
        .ok_or_else(AdminApiError::provider_oauth_unavailable)?;
    let result = service.start(endpoint_id, request).await?;
    Ok(no_store::json(ProviderOAuthStartResponse::from(result)))
}

pub(crate) async fn exchange(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    payload: Result<Json<ProviderOAuthExchangeBody>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let endpoint_id = parse_endpoint_id(&endpoint_id)?;
    let (session_id, callback_url) = parse_json(payload)?.into_parts();
    let service = state
        .provider_oauth()
        .ok_or_else(AdminApiError::provider_oauth_unavailable)?;
    let result = service
        .exchange(endpoint_id, &session_id, &callback_url)
        .await?;
    Ok(no_store::json(ProviderOAuthExchangeResponse::from(result)))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn parse_endpoint_id(value: &str) -> Result<ProviderEndpointId, AdminApiError> {
    ProviderEndpointId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("provider endpoint id is invalid"))
}
