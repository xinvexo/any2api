use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::{HttpAccessLogCapacity, writes::delete_oldest};

pub(super) async fn trim_to_capacity(
    connection: &mut SqliteConnection,
    capacity: HttpAccessLogCapacity,
) -> Result<u64, StorageError> {
    let (row_count, exchange_bytes) = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*), COALESCE(SUM(exchange_bytes), 0) \
             FROM http_access_logs INDEXED BY http_access_logs_retention_idx",
    )
    .fetch_one(&mut *connection)
    .await?;
    let row_count = to_u64(row_count)?;
    let exchange_bytes = to_u64(exchange_bytes)?;
    let excess_rows = row_count.saturating_sub(capacity.max_rows());
    let excess_bytes = exchange_bytes.saturating_sub(capacity.max_exchange_bytes());
    if excess_rows == 0 && excess_bytes == 0 {
        return Ok(0);
    }

    let delete_count: Option<i64> = sqlx::query_scalar(
        "SELECT delete_count FROM (\
             SELECT ROW_NUMBER() OVER (ORDER BY started_at_ms ASC, request_id ASC) \
                        AS delete_count, \
                    SUM(exchange_bytes) OVER (ORDER BY started_at_ms ASC, request_id ASC \
                        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS freed_bytes \
             FROM http_access_logs INDEXED BY http_access_logs_retention_idx\
         ) WHERE delete_count >= ? AND freed_bytes >= ? \
         ORDER BY delete_count ASC LIMIT 1",
    )
    .bind(to_i64(excess_rows)?)
    .bind(to_i64(excess_bytes)?)
    .fetch_optional(&mut *connection)
    .await?;
    let delete_count = delete_count.ok_or(StorageError::CorruptTelemetry)?;
    delete_oldest(connection, to_u64(delete_count)?).await
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
