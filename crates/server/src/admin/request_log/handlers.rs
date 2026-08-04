use std::str::FromStr;

use any2api_domain::RequestId;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};

use crate::{
    log_pagination::{LogCursorKind, LogListQuery},
    state::AppState,
};

use super::{
    dto::{RequestLogDetailResponse, RequestLogListResponse},
    error::AdminApiError,
};

pub(crate) async fn list(
    State(state): State<AppState>,
    query: Result<Query<LogListQuery>, QueryRejection>,
) -> Result<Json<RequestLogListResponse>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("request log query is invalid"))?
        .0
        .validate(LogCursorKind::Request)
        .ok_or_else(|| AdminApiError::invalid_request("request log page is invalid"))?;
    let telemetry = state.request_telemetry();
    let logs = telemetry
        .list(query.since_ms, query.cursor, query.page_size)
        .await
        .map_err(|error| {
            tracing::error!(%error, "request log list failed");
            AdminApiError::request_log_unavailable()
        })?;
    let snapshot = state.snapshots().load();
    Ok(Json(RequestLogListResponse::new(
        logs,
        query.page_size,
        telemetry.metrics(),
        snapshot.as_ref(),
    )))
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
    let snapshot = state.snapshots().load();
    Ok(Json(RequestLogDetailResponse::new(
        record,
        telemetry.metrics(),
        snapshot.as_ref(),
    )))
}
