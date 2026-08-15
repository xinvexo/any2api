use any2api_domain::{HttpAccessLogSummary, LogPage};
use any2api_runtime::api::RequestTelemetryMetrics;
use serde::Serialize;

use crate::log_pagination::LogCursorScope;

#[derive(Serialize)]
pub(super) struct SystemLogListResponse {
    items: Vec<SystemLogResponse>,
    total: u64,
    page: u32,
    page_size: u32,
    cursor: Option<String>,
    next_cursor: Option<String>,
    telemetry: TelemetryResponse,
}

impl SystemLogListResponse {
    pub(super) fn new(
        logs: LogPage<HttpAccessLogSummary>,
        page_size: u32,
        metrics: RequestTelemetryMetrics,
        show_admin_operations: bool,
    ) -> Self {
        let cursor = logs
            .cursor
            .as_ref()
            .map(|cursor| LogCursorScope::System(show_admin_operations).encode(cursor, logs.page));
        let next_cursor = logs.next_cursor.as_ref().map(|cursor| {
            LogCursorScope::System(show_admin_operations)
                .encode(cursor, logs.page.saturating_add(1))
        });
        Self {
            items: logs
                .items
                .into_iter()
                .map(SystemLogResponse::from)
                .collect(),
            total: logs.total,
            page: logs.page,
            page_size,
            cursor,
            next_cursor,
            telemetry: metrics.into(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct ClearSystemLogsResponse {
    deleted: u64,
}

impl ClearSystemLogsResponse {
    pub(super) const fn new(deleted: u64) -> Self {
        Self { deleted }
    }
}

#[derive(Serialize)]
struct TelemetryResponse {
    queued_records: usize,
    in_flight_records: usize,
    dropped_records: u64,
    persisted_records: u64,
}

impl From<RequestTelemetryMetrics> for TelemetryResponse {
    fn from(value: RequestTelemetryMetrics) -> Self {
        Self {
            queued_records: value.queued_records,
            in_flight_records: value.in_flight_records,
            dropped_records: value.dropped_records,
            persisted_records: value.persisted_records,
        }
    }
}

#[derive(Serialize)]
pub(super) struct SystemLogResponse {
    request_id: String,
    started_at_ms: u64,
    config_revision: u64,
    client_ip: Option<String>,
    method: String,
    path: String,
    uri: String,
    http_version: &'static str,
    status_code: Option<u16>,
    duration_ms: u64,
    response_bytes: u64,
    outcome: &'static str,
    exchange_captured: bool,
}

impl From<HttpAccessLogSummary> for SystemLogResponse {
    fn from(value: HttpAccessLogSummary) -> Self {
        Self {
            request_id: value.request_id.to_string(),
            started_at_ms: value.started_at_ms,
            config_revision: value.config_revision.get(),
            client_ip: value.client_ip.map(|address| address.to_string()),
            method: value.method,
            path: value.path,
            uri: value.uri,
            http_version: value.http_version.as_str(),
            status_code: value.status_code,
            duration_ms: value.duration_ms,
            response_bytes: value.response_bytes,
            outcome: value.outcome.as_str(),
            exchange_captured: value.exchange_captured,
        }
    }
}
