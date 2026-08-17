use std::str::FromStr;

use any2api_domain::RequestId;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};

use crate::{admin::AdminApiError, log_cursor::LOG_BATCH_SIZE, state::AppState};

use super::{
    detail_dto::SystemLogDetailResponse,
    dto::{ClearSystemLogsResponse, SystemLogListResponse},
    query::SystemLogListQuery,
};

pub(super) async fn list(
    State(state): State<AppState>,
    query: Result<Query<SystemLogListQuery>, QueryRejection>,
) -> Result<Json<SystemLogListResponse>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("system log query is invalid"))?
        .0
        .validate()
        .ok_or_else(|| AdminApiError::invalid_request("system log cursor is invalid"))?;
    let telemetry = state.request_telemetry();
    let logs = telemetry
        .list_http_access_logs(
            query.batch.since_ms,
            query.show_admin_operations,
            query.batch.cursor,
            LOG_BATCH_SIZE,
        )
        .await
        .map_err(|error| {
            tracing::error!(%error, "system log list failed");
            AdminApiError::system_log_unavailable()
        })?;
    Ok(Json(SystemLogListResponse::new(
        logs,
        telemetry.metrics(),
        query.show_admin_operations,
    )))
}

pub(super) async fn get(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
) -> Result<Json<SystemLogDetailResponse>, AdminApiError> {
    let request_id = RequestId::from_str(&request_id)
        .map_err(|_| AdminApiError::invalid_request("request ID is invalid"))?;
    let record = state
        .request_telemetry()
        .get_http_access_log(request_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, "system log detail failed");
            AdminApiError::system_log_unavailable()
        })?
        .ok_or_else(AdminApiError::system_log_not_found)?;
    Ok(Json(record.into()))
}

pub(super) async fn clear(
    State(state): State<AppState>,
) -> Result<Json<ClearSystemLogsResponse>, AdminApiError> {
    let deleted = state
        .request_telemetry()
        .clear_http_access_logs()
        .await
        .map_err(|error| {
            tracing::error!(%error, "system log clear failed");
            AdminApiError::system_log_unavailable()
        })?;
    Ok(Json(ClearSystemLogsResponse::new(deleted)))
}
