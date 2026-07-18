use std::str::FromStr;

use any2api_domain::ModelRouteId;
use any2api_runtime::api::ConfigPublishError;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    model_route_dto::{
        ModelRouteCollectionResponse, ModelRouteDeleteQuery, ModelRouteWriteRequest,
    },
};

pub(crate) async fn list(State(state): State<AppState>) -> Json<ModelRouteCollectionResponse> {
    let snapshot = state.snapshots().load();
    Json(ModelRouteCollectionResponse::from_snapshot(&snapshot))
}

pub(crate) async fn create(
    State(state): State<AppState>,
    payload: Result<Json<ModelRouteWriteRequest>, JsonRejection>,
) -> Result<Json<ModelRouteCollectionResponse>, AdminApiError> {
    let (expected, draft) = parse_json(payload)?.into_create_domain()?;
    let snapshot = state
        .publisher()
        .create_model_route(expected, ModelRouteId::new(), draft)
        .await?;
    Ok(Json(ModelRouteCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    payload: Result<Json<ModelRouteWriteRequest>, JsonRejection>,
) -> Result<Json<ModelRouteCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let request = parse_json(payload)?;
    let expected = request.revision()?;
    let current = state.snapshots().load();
    if current.revision() != expected {
        return Err(ConfigPublishError::RevisionConflict {
            expected,
            actual: current.revision(),
        }
        .into());
    }
    let existing = current
        .model_routes()
        .get(id)
        .ok_or_else(AdminApiError::model_route_not_found)?;
    let (expected, expected_config_version, draft) = request.into_update_domain(existing)?;
    let snapshot = state
        .publisher()
        .update_model_route(expected, id, expected_config_version, draft)
        .await?;
    Ok(Json(ModelRouteCollectionResponse::from_snapshot(&snapshot)))
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    query: Result<Query<ModelRouteDeleteQuery>, QueryRejection>,
) -> Result<Json<ModelRouteCollectionResponse>, AdminApiError> {
    let id = parse_id(&id)?;
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
        .delete_model_route(expected, id, expected_config_version)
        .await?;
    Ok(Json(ModelRouteCollectionResponse::from_snapshot(&snapshot)))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn parse_id(value: &str) -> Result<ModelRouteId, AdminApiError> {
    ModelRouteId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("model route id is invalid"))
}
