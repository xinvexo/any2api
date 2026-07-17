use std::str::FromStr;

use any2api_domain::ProxyProfileId;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    proxy_dto::{
        ExpectedRevisionQuery, ExpectedRevisionRequest, ProxyCollectionResponse, ProxyWriteRequest,
    },
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<ProxyCollectionResponse> {
    let snapshot = state.snapshots().load();
    Json(ProxyCollectionResponse::from_snapshot(&snapshot))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    payload: Result<Json<ProxyWriteRequest>, JsonRejection>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let request = parse_json(payload)?;
    let (expected, draft) = request.into_domain()?;
    let snapshot = state
        .publisher()
        .create_proxy(expected, ProxyProfileId::new(), draft)
        .await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<ProxyWriteRequest>, JsonRejection>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let request = parse_json(payload)?;
    let (expected, draft) = request.into_domain()?;
    let snapshot = state.publisher().update_proxy(expected, id, draft).await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    query: Result<Query<ExpectedRevisionQuery>, QueryRejection>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let expected = query
        .map_err(|_| AdminApiError::invalid_request("expected_revision query is required"))?
        .0
        .revision()?;
    let snapshot = state.publisher().delete_proxy(expected, id).await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn set_global(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<ExpectedRevisionRequest>, JsonRejection>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let expected = parse_json(payload)?.revision()?;
    let snapshot = state.publisher().set_global_proxy(expected, id).await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn parse_id(value: &str) -> Result<ProxyProfileId, AdminApiError> {
    ProxyProfileId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("proxy id is invalid"))
}
