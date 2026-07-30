use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};

use crate::{admin::AdminApiError, log_pagination::LogListQuery, state::AppState};

use super::dto::{ClearSystemLogsResponse, SystemLogListResponse};

pub(super) async fn list(
    State(state): State<AppState>,
    query: Result<Query<LogListQuery>, QueryRejection>,
) -> Result<Json<SystemLogListResponse>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("system log query is invalid"))?
        .0
        .validate()
        .ok_or_else(|| AdminApiError::invalid_request("system log page is invalid"))?;
    let telemetry = state.request_telemetry();
    let logs = telemetry
        .list_http_access_logs(query.since_ms, query.offset, query.page_size)
        .await
        .map_err(|error| {
            tracing::error!(%error, "system log list failed");
            AdminApiError::system_log_unavailable()
        })?;
    Ok(Json(SystemLogListResponse::new(
        logs,
        query.page,
        query.page_size,
        telemetry.metrics(),
    )))
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
