use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::HeaderMap,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{admin::AdminApiError, state::AppState};

use super::{
    dto::{ClearSystemLogsResponse, SystemLogListResponse},
    middleware::ExcludeFromHttpAccessLog,
};

const REFRESH_KIND_HEADER: &str = "x-any2api-system-log-refresh";
const AUTOMATIC_REFRESH: &[u8] = b"automatic";

#[derive(Deserialize)]
pub(super) struct SystemLogListQuery {
    limit: Option<u32>,
}

pub(super) async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    query: Result<Query<SystemLogListQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("system log query is invalid"))?
        .0;
    let limit = query.limit.unwrap_or(200);
    if !(1..=500).contains(&limit) {
        return Err(AdminApiError::invalid_request(
            "system log limit must be between 1 and 500",
        ));
    }
    let telemetry = state.request_telemetry();
    let logs = telemetry
        .list_http_access_logs(limit)
        .await
        .map_err(|error| {
            tracing::error!(%error, "system log list failed");
            AdminApiError::system_log_unavailable()
        })?;
    let mut response = Json(SystemLogListResponse::new(logs, telemetry.metrics())).into_response();
    if headers
        .get(REFRESH_KIND_HEADER)
        .is_some_and(|value| value.as_bytes() == AUTOMATIC_REFRESH)
    {
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
