use std::str::FromStr;

use any2api_domain::RequestId;
use any2api_runtime::api::ActiveRequestLogBatch;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};

use crate::{log_cursor::LOG_BATCH_SIZE, state::AppState};

use super::{
    dto::{RequestLogDetailResponse, RequestLogListResponse},
    error::AdminApiError,
    query::RequestLogListQuery,
};

pub(crate) async fn list(
    State(state): State<AppState>,
    query: Result<Query<RequestLogListQuery>, QueryRejection>,
) -> Result<Json<RequestLogListResponse>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("request log query is invalid"))?
        .0
        .validate()
        .ok_or_else(|| AdminApiError::invalid_request("request log cursor is invalid"))?;
    let telemetry = state.request_telemetry();
    let active = if query.batch.cursor.is_none() {
        telemetry.list_active_requests(&query.filter, LOG_BATCH_SIZE)
    } else {
        ActiveRequestLogBatch::empty()
    };
    let logs = telemetry
        .list(
            query.batch.since_ms,
            &query.filter,
            query.batch.cursor,
            LOG_BATCH_SIZE,
        )
        .await
        .map_err(|error| {
            tracing::error!(%error, "request log list failed");
            AdminApiError::request_log_unavailable()
        })?;
    let snapshot = state.snapshots().load();
    Ok(Json(RequestLogListResponse::new(
        logs,
        active,
        telemetry.metrics(),
        snapshot.as_ref(),
        &query.filter_fingerprint,
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
