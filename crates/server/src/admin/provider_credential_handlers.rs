use std::str::FromStr;

use any2api_domain::{CredentialId, ProviderEndpointId};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    response::Response,
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    no_store,
    provider_credential_dto::{
        ProviderCredentialCollectionResponse, ProviderCredentialCreateRequest,
        ProviderCredentialDeleteQuery, ProviderCredentialRotateRequest,
        ProviderCredentialUpdateRequest,
    },
};

pub(crate) async fn list(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
) -> Result<Response, AdminApiError> {
    let endpoint_id = parse_endpoint_id(&endpoint_id)?;
    let snapshot = state.snapshots().load();
    require_endpoint(&snapshot, endpoint_id)?;
    Ok(response(&snapshot, endpoint_id))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    Path(endpoint_id): Path<String>,
    payload: Result<Json<ProviderCredentialCreateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let endpoint_id = parse_endpoint_id(&endpoint_id)?;
    let (expected, draft, api_key) = parse_json(payload)?.into_domain()?;
    let snapshot = state
        .publisher()
        .create_provider_credential(expected, CredentialId::new(), endpoint_id, draft, api_key)
        .await?;
    Ok(response(&snapshot, endpoint_id))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<ProviderCredentialUpdateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_credential_id(&id)?;
    let (expected, expected_config_version, draft) = parse_json(payload)?.into_domain()?;
    let endpoint_id = credential_endpoint(&state, id)?;
    let snapshot = state
        .publisher()
        .update_provider_credential(expected, id, expected_config_version, draft)
        .await?;
    Ok(response(&snapshot, endpoint_id))
}

pub(crate) async fn rotate_secret(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<ProviderCredentialRotateRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_credential_id(&id)?;
    let (expected, expected_config_version, expected_secret_version, api_key) =
        parse_json(payload)?.into_domain()?;
    let endpoint_id = credential_endpoint(&state, id)?;
    let snapshot = state
        .publisher()
        .rotate_provider_credential_secret(
            expected,
            id,
            expected_config_version,
            expected_secret_version,
            api_key,
        )
        .await?;
    Ok(response(&snapshot, endpoint_id))
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    query: Result<Query<ProviderCredentialDeleteQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
    let id = parse_credential_id(&id)?;
    let endpoint_id = credential_endpoint(&state, id)?;
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
        .delete_provider_credential(expected, id, expected_config_version)
        .await?;
    Ok(response(&snapshot, endpoint_id))
}

fn response(
    snapshot: &any2api_runtime::api::PublishedSnapshot,
    endpoint_id: ProviderEndpointId,
) -> Response {
    no_store::json(ProviderCredentialCollectionResponse::from_snapshot(
        snapshot,
        endpoint_id,
    ))
}

fn credential_endpoint(
    state: &AppState,
    id: CredentialId,
) -> Result<ProviderEndpointId, AdminApiError> {
    state
        .snapshots()
        .load()
        .provider_credentials()
        .get(id)
        .map(|credential| credential.provider_endpoint_id())
        .ok_or_else(AdminApiError::provider_credential_not_found)
}

fn require_endpoint(
    snapshot: &any2api_runtime::api::PublishedSnapshot,
    id: ProviderEndpointId,
) -> Result<(), AdminApiError> {
    snapshot
        .provider_endpoints()
        .get(id)
        .map(|_| ())
        .ok_or_else(AdminApiError::provider_endpoint_not_found)
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

fn parse_credential_id(value: &str) -> Result<CredentialId, AdminApiError> {
    CredentialId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("provider credential id is invalid"))
}
