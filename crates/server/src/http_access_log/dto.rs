use any2api_domain::HttpAccessLog;
use any2api_runtime::api::RequestTelemetryMetrics;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct SystemLogListResponse {
    items: Vec<SystemLogResponse>,
    telemetry: TelemetryResponse,
}

impl SystemLogListResponse {
    pub(super) fn new(logs: Vec<HttpAccessLog>, metrics: RequestTelemetryMetrics) -> Self {
        Self {
            items: logs.into_iter().map(SystemLogResponse::from).collect(),
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
    dropped_records: u64,
    persisted_records: u64,
}

impl From<RequestTelemetryMetrics> for TelemetryResponse {
    fn from(value: RequestTelemetryMetrics) -> Self {
        Self {
            queued_records: value.queued_records,
            dropped_records: value.dropped_records,
            persisted_records: value.persisted_records,
        }
    }
}

#[derive(Serialize)]
struct SystemLogResponse {
    request_id: String,
    started_at_ms: u64,
    config_revision: u64,
    client_ip: Option<String>,
    method: String,
    path: String,
    http_version: &'static str,
    status_code: Option<u16>,
    duration_ms: u64,
    response_bytes: u64,
    outcome: &'static str,
}

impl From<HttpAccessLog> for SystemLogResponse {
    fn from(value: HttpAccessLog) -> Self {
        Self {
            request_id: value.request_id.to_string(),
            started_at_ms: value.started_at_ms,
            config_revision: value.config_revision.get(),
            client_ip: value.client_ip.map(|address| address.to_string()),
            method: value.method,
            path: value.path,
            http_version: value.http_version.as_str(),
            status_code: value.status_code,
            duration_ms: value.duration_ms,
            response_bytes: value.response_bytes,
            outcome: value.outcome.as_str(),
        }
    }
}
