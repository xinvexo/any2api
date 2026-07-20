use std::str::FromStr;

use any2api_domain::RequestId;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use serde::Deserialize;

use crate::state::AppState;

use super::{
    error::AdminApiError,
    request_log_dto::{RequestLogDetailResponse, RequestLogListResponse},
};

#[derive(Deserialize)]
pub(crate) struct RequestLogListQuery {
    limit: Option<u32>,
}

pub(crate) async fn list(
    State(state): State<AppState>,
    query: Result<Query<RequestLogListQuery>, QueryRejection>,
) -> Result<Json<RequestLogListResponse>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("request log query is invalid"))?
        .0;
    let limit = query.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return Err(AdminApiError::invalid_request(
            "request log limit must be between 1 and 200",
        ));
    }
    let telemetry = state.request_telemetry();
    let logs = telemetry.list(limit).await.map_err(|error| {
        tracing::error!(%error, "request log list failed");
        AdminApiError::request_log_unavailable()
    })?;
    Ok(Json(RequestLogListResponse::new(logs, telemetry.metrics())))
}

pub(crate) async fn get(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
) -> Result<Json<RequestLogDetailResponse>, AdminApiError> {
    let request_id = RequestId::from_str(&request_id)
        .map_err(|_| AdminApiError::invalid_request("request ID is invalid"))?;
    let telemetry = state.request_telemetry();
    let record = telemetry.get(request_id).await.map_err(|error| {
        tracing::error!(%error, "request log detail failed");
        AdminApiError::request_log_unavailable()
    })?;
    let record = record.ok_or_else(AdminApiError::request_log_not_found)?;
    Ok(Json(RequestLogDetailResponse::new(
        record,
        telemetry.metrics(),
    )))
}
