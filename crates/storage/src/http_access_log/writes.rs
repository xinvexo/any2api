use any2api_domain::{HttpAccessLog, HttpBodyCapture, HttpHeader, canonical_ip};
use sqlx::SqliteConnection;

use crate::error::StorageError;

pub(super) async fn insert(
    connection: &mut SqliteConnection,
    log: &HttpAccessLog,
) -> Result<(), StorageError> {
    let empty_body = HttpBodyCapture {
        content: Vec::new(),
        total_bytes: 0,
        complete: false,
        truncated: false,
    };
    let empty_headers: &[HttpHeader] = &[];
    let exchange = log.exchange.as_ref();
    let request_headers = serialize_headers(
        exchange.map_or(empty_headers, |value| value.request_headers.as_slice()),
    )?;
    let response_headers = serialize_headers(
        exchange.map_or(empty_headers, |value| value.response_headers.as_slice()),
    )?;
    let request_body = exchange.map_or(&empty_body, |value| &value.request_body);
    let response_body = exchange.map_or(&empty_body, |value| &value.response_body);
    let exchange_bytes = if exchange.is_some() {
        [
            request_headers.len(),
            request_body.content.len(),
            response_headers.len(),
            response_body.content.len(),
        ]
        .into_iter()
        .try_fold(0_u64, |total, bytes| {
            total.checked_add(u64::try_from(bytes).ok()?)
        })
        .ok_or(StorageError::CorruptTelemetry)?
    } else {
        0
    };
    sqlx::query(
        "INSERT INTO http_access_logs (request_id, started_at_ms, config_revision, client_ip, \
         method, path, uri, http_version, status_code, duration_ms, response_bytes, outcome, \
         exchange_captured, request_headers, request_body, request_body_bytes, \
         request_body_complete, request_body_truncated, response_headers, response_body, \
         response_body_complete, response_body_truncated, exchange_bytes) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(log.request_id.to_string())
    .bind(to_i64(log.started_at_ms)?)
    .bind(to_i64(log.config_revision.get())?)
    .bind(
        log.client_ip
            .map(canonical_ip)
            .map(|address| address.to_string()),
    )
    .bind(&log.method)
    .bind(&log.path)
    .bind(&log.uri)
    .bind(log.http_version.as_str())
    .bind(log.status_code.map(i64::from))
    .bind(to_i64(log.duration_ms)?)
    .bind(to_i64(log.response_bytes)?)
    .bind(log.outcome.as_str())
    .bind(bool_value(exchange.is_some()))
    .bind(request_headers)
    .bind(&request_body.content)
    .bind(to_i64(request_body.total_bytes)?)
    .bind(bool_value(request_body.complete))
    .bind(bool_value(request_body.truncated))
    .bind(response_headers)
    .bind(&response_body.content)
    .bind(bool_value(response_body.complete))
    .bind(bool_value(response_body.truncated))
    .bind(to_i64(exchange_bytes)?)
    .execute(connection)
    .await?;
    Ok(())
}

fn serialize_headers(headers: &[HttpHeader]) -> Result<Vec<u8>, StorageError> {
    serde_json::to_vec(headers).map_err(|_| StorageError::CorruptTelemetry)
}

const fn bool_value(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

pub(super) async fn delete_oldest_before(
    connection: &mut SqliteConnection,
    cutoff_ms: u64,
    limit: u64,
) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM http_access_logs WHERE request_id IN (SELECT request_id \
         FROM http_access_logs INDEXED BY http_access_logs_retention_idx WHERE started_at_ms < ? \
         ORDER BY started_at_ms ASC, request_id ASC LIMIT ?)",
    )
    .bind(to_i64(cutoff_ms)?)
    .bind(to_i64(limit)?)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

pub(super) async fn delete_oldest(
    connection: &mut SqliteConnection,
    limit: u64,
) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM http_access_logs WHERE request_id IN (SELECT request_id \
         FROM http_access_logs INDEXED BY http_access_logs_retention_idx \
         ORDER BY started_at_ms ASC, request_id ASC LIMIT ?)",
    )
    .bind(to_i64(limit)?)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
