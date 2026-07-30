use std::str::FromStr;

use any2api_domain::RequestId;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
    http::HeaderMap,
    response::{IntoResponse, Response},
};

use crate::{
    http_access_log::{ExcludeFromHttpAccessLog, is_automatic_log_refresh},
    log_pagination::LogListQuery,
    state::AppState,
};

use super::{
    dto::{RequestLogDetailResponse, RequestLogListResponse},
    error::AdminApiError,
};

pub(crate) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<LogListQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("request log query is invalid"))?
        .0
        .validate()
        .ok_or_else(|| AdminApiError::invalid_request("request log page is invalid"))?;
    let telemetry = state.request_telemetry();
    let logs = telemetry
        .list(query.since_ms, query.offset, query.page_size)
        .await
        .map_err(|error| {
            tracing::error!(%error, "request log list failed");
            AdminApiError::request_log_unavailable()
        })?;
    let snapshot = state.snapshots().load();
    let mut response = Json(RequestLogListResponse::new(
        logs,
        query.page,
        query.page_size,
        telemetry.metrics(),
        snapshot.as_ref(),
    ))
    .into_response();
    if is_automatic_log_refresh(&headers) {
        response.extensions_mut().insert(ExcludeFromHttpAccessLog);
    }
    Ok(response)
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
