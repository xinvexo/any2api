use any2api_domain::HttpAccessLog;
use sqlx::SqliteConnection;

use crate::error::StorageError;

pub(super) async fn insert(
    connection: &mut SqliteConnection,
    log: &HttpAccessLog,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO http_access_logs (request_id, started_at_ms, config_revision, client_ip, \
         method, path, http_version, status_code, duration_ms, response_bytes, outcome) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(log.request_id.to_string())
    .bind(to_i64(log.started_at_ms)?)
    .bind(to_i64(log.config_revision.get())?)
    .bind(log.client_ip.map(|address| address.to_string()))
    .bind(&log.method)
    .bind(&log.path)
    .bind(log.http_version.as_str())
    .bind(log.status_code.map(i64::from))
    .bind(to_i64(log.duration_ms)?)
    .bind(to_i64(log.response_bytes)?)
    .bind(log.outcome.as_str())
    .execute(connection)
    .await?;
    Ok(())
}

pub(super) async fn delete_oldest_before(
    connection: &mut SqliteConnection,
    cutoff_ms: u64,
    limit: u64,
) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM http_access_logs WHERE request_id IN (SELECT request_id \
         FROM http_access_logs WHERE started_at_ms < ? \
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
         FROM http_access_logs ORDER BY started_at_ms ASC, request_id ASC LIMIT ?)",
    )
    .bind(to_i64(limit)?)
    .execute(connection)
    .await?;
    Ok(result.rows_affected())
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
