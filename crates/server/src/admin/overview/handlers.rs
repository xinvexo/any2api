use any2api_runtime::api::RequestLogOverviewRange;
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use serde::Deserialize;

use crate::{admin::AdminApiError, state::AppState};

use super::dto::OverviewResourcesResponse;
use super::dto::OverviewUsageResponse;

#[derive(Deserialize)]
pub(crate) struct OverviewUsageQuery {
    range: Option<String>,
}

pub(crate) async fn usage(
    State(state): State<AppState>,
    query: Result<Query<OverviewUsageQuery>, QueryRejection>,
) -> Result<Json<OverviewUsageResponse>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("overview query is invalid"))?
        .0;
    let range = parse_range(query.range.as_deref())?;
    let overview = state
        .request_telemetry()
        .overview_usage(range)
        .await
        .map_err(|error| {
            tracing::error!(%error, "overview usage statistics failed");
            AdminApiError::request_log_unavailable()
        })?;
    Ok(Json(overview.into()))
}

pub(crate) async fn resources(
    State(state): State<AppState>,
) -> Result<Json<OverviewResourcesResponse>, AdminApiError> {
    let snapshot = state
        .admin_realtime()
        .current_snapshot()
        .await
        .ok_or_else(|| {
            tracing::warn!("shared system resource snapshot is unavailable");
            AdminApiError::system_metrics_unavailable()
        })?;
    Ok(Json(snapshot.resources()))
}

fn parse_range(value: Option<&str>) -> Result<RequestLogOverviewRange, AdminApiError> {
    match value.unwrap_or("24h") {
        "1h" => Ok(RequestLogOverviewRange::OneHour),
        "24h" => Ok(RequestLogOverviewRange::TwentyFourHours),
        "7d" => Ok(RequestLogOverviewRange::SevenDays),
        "30d" => Ok(RequestLogOverviewRange::ThirtyDays),
        _ => Err(AdminApiError::invalid_request(
            "overview range must be one of 1h, 24h, 7d, or 30d",
        )),
    }
}
