use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogOutcome, HttpProtocolVersion, RequestId,
};
use sqlx::FromRow;

use crate::error::StorageError;

#[derive(FromRow)]
pub(super) struct HttpAccessLogRow {
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

pub(super) fn parse(row: HttpAccessLogRow) -> Result<HttpAccessLog, StorageError> {
    Ok(HttpAccessLog {
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

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
