use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::HeaderMap,
    response::{IntoResponse, Response},
};

use crate::{admin::AdminApiError, log_pagination::LogListQuery, state::AppState};

use super::{
    dto::{ClearSystemLogsResponse, SystemLogListResponse},
    is_automatic_log_refresh,
    middleware::ExcludeFromHttpAccessLog,
};

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<LogListQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
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
    let mut response = Json(SystemLogListResponse::new(
        logs,
        query.page,
        query.page_size,
        telemetry.metrics(),
    ))
    .into_response();
    if is_automatic_log_refresh(&headers) {
        response.extensions_mut().insert(ExcludeFromHttpAccessLog);
    }
    Ok(response)
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
