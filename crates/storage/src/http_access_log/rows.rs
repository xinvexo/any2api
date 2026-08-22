use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogOutcome, HttpAccessLogSummary, HttpProtocolVersion,
    LogCursorPosition, RequestId,
};
use sqlx::FromRow;

use crate::error::StorageError;

#[derive(FromRow)]
pub(super) struct HttpAccessLogSummaryRow {
    request_id: String,
    started_at_ms: i64,
    config_revision: i64,
    client_ip: Option<String>,
    method: String,
    path: String,
    http_version: String,
    status_code: Option<i64>,
    duration_ms: i64,
    response_bytes: i64,
    outcome: String,
}

impl HttpAccessLogSummaryRow {
    pub(super) fn cursor_position(&self) -> Result<LogCursorPosition, StorageError> {
        Ok(LogCursorPosition::new(
            to_u64(self.started_at_ms)?,
            self.request_id.clone(),
        ))
    }
}

#[derive(FromRow)]
pub(super) struct HttpAccessLogDetailRow {
    #[sqlx(flatten)]
    summary: HttpAccessLogSummaryRow,
    gateway_auth_rejected: i64,
}

pub(super) fn parse_summary(
    row: HttpAccessLogSummaryRow,
) -> Result<HttpAccessLogSummary, StorageError> {
    Ok(HttpAccessLogSummary {
        request_id: row
            .request_id
            .parse::<RequestId>()
            .map_err(|_| StorageError::CorruptTelemetry)?,
        started_at_ms: to_u64(row.started_at_ms)?,
        config_revision: ConfigRevision::new(to_u64(row.config_revision)?)
            .map_err(|_| StorageError::CorruptTelemetry)?,
        client_ip: row
            .client_ip
            .map(|value| value.parse().map_err(|_| StorageError::CorruptTelemetry))
            .transpose()?,
        method: row.method,
        path: row.path,
        http_version: HttpProtocolVersion::parse(&row.http_version)
            .ok_or(StorageError::CorruptTelemetry)?,
        status_code: row
            .status_code
            .map(u16::try_from)
            .transpose()
            .map_err(|_| StorageError::CorruptTelemetry)?,
        duration_ms: to_u64(row.duration_ms)?,
        response_bytes: to_u64(row.response_bytes)?,
        outcome: HttpAccessLogOutcome::parse(&row.outcome).ok_or(StorageError::CorruptTelemetry)?,
    })
}

pub(super) fn parse_detail(row: HttpAccessLogDetailRow) -> Result<HttpAccessLog, StorageError> {
    let summary = parse_summary(row.summary)?;
    Ok(HttpAccessLog {
        request_id: summary.request_id,
        started_at_ms: summary.started_at_ms,
        config_revision: summary.config_revision,
        client_ip: summary.client_ip,
        method: summary.method,
        path: summary.path,
        http_version: summary.http_version,
        status_code: summary.status_code,
        duration_ms: summary.duration_ms,
        response_bytes: summary.response_bytes,
        outcome: summary.outcome,
        gateway_auth_rejected: parse_bool(row.gateway_auth_rejected)?,
    })
}

fn parse_bool(value: i64) -> Result<bool, StorageError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StorageError::CorruptTelemetry),
    }
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
