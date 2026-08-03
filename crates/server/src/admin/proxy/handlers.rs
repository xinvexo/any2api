use std::str::FromStr;

use any2api_domain::ProxyProfileId;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
};

use crate::state::AppState;

use super::{
    dto::{
        ProxyAuthenticationRequest, ProxyCollectionResponse, ProxyTestResponse, ProxyWriteRequest,
    },
    error::AdminApiError,
    request_json::AdminJson,
    revision::{ExpectedRevisionRequest, RequiredRevisionQuery},
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<ProxyCollectionResponse> {
    let snapshot = state.snapshots().load();
    Json(ProxyCollectionResponse::from_snapshot(&snapshot))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    AdminJson(request): AdminJson<ProxyWriteRequest>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
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
    AdminJson(request): AdminJson<ProxyWriteRequest>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, draft) = request.into_domain()?;
    let snapshot = state.publisher().update_proxy(expected, id, draft).await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RequiredRevisionQuery(expected): RequiredRevisionQuery,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let snapshot = state.publisher().delete_proxy(expected, id).await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn set_global(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(payload): AdminJson<ExpectedRevisionRequest>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let expected = payload.revision()?;
    let snapshot = state.publisher().set_global_proxy(expected, id).await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn set_authentication(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(payload): AdminJson<ProxyAuthenticationRequest>,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, username, password) = payload.into_domain()?;
    let snapshot = state
        .publisher()
        .set_proxy_authentication(expected, id, username, password)
        .await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn clear_authentication(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RequiredRevisionQuery(expected): RequiredRevisionQuery,
) -> Result<Json<ProxyCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let snapshot = state
        .publisher()
        .clear_proxy_authentication(expected, id)
        .await?;

    Ok(Json(ProxyCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn test(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ProxyTestResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    if !body.is_empty() {
        return Err(AdminApiError::invalid_request(
            "proxy test request body must be empty",
        ));
    }
    let service = state
        .proxy_tests()
        .ok_or_else(AdminApiError::proxy_test_unavailable)?;
    let result = service.test(state.snapshots().load(), id).await?;
    Ok(Json(result.into()))
}

fn parse_id(value: &str) -> Result<ProxyProfileId, AdminApiError> {
    ProxyProfileId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("proxy id is invalid"))
}
