use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::{
    HttpAccessLogCapacity,
    writes::{delete_oldest, delete_oldest_gateway_auth_rejected},
};

#[derive(Clone, Copy)]
struct CapacityStats {
    rows: u64,
    exchange_bytes: u64,
    gateway_auth_rejected_rows: u64,
    gateway_auth_rejected_exchange_bytes: u64,
}

pub(super) async fn trim_to_capacity(
    connection: &mut SqliteConnection,
    capacity: HttpAccessLogCapacity,
) -> Result<u64, StorageError> {
    let stats = capacity_stats(connection).await?;
    let rejected_rows_to_free = stats
        .gateway_auth_rejected_rows
        .saturating_sub(capacity.gateway_auth_rejected_max_rows())
        .max(stats.rows.saturating_sub(capacity.max_rows()))
        .min(stats.gateway_auth_rejected_rows);
    let rejected_bytes_to_free = stats
        .gateway_auth_rejected_exchange_bytes
        .saturating_sub(capacity.gateway_auth_rejected_max_exchange_bytes())
        .max(
            stats
                .exchange_bytes
                .saturating_sub(capacity.max_exchange_bytes()),
        )
        .min(stats.gateway_auth_rejected_exchange_bytes);

    let mut deleted = 0;
    if rejected_rows_to_free > 0 || rejected_bytes_to_free > 0 {
        let delete_count = gateway_auth_rejected_delete_count(
            connection,
            rejected_rows_to_free,
            rejected_bytes_to_free,
        )
        .await?;
        deleted = delete_oldest_gateway_auth_rejected(connection, delete_count).await?;
    }
    let remaining = if deleted == 0 {
        stats
    } else {
        capacity_stats(connection).await?
    };
    Ok(deleted.saturating_add(trim_global_capacity(connection, capacity, remaining).await?))
}

async fn capacity_stats(connection: &mut SqliteConnection) -> Result<CapacityStats, StorageError> {
    let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT COUNT(*), COALESCE(SUM(exchange_bytes), 0), \
                COALESCE(SUM(gateway_auth_rejected), 0), \
                COALESCE(SUM(CASE gateway_auth_rejected \
                    WHEN 1 THEN exchange_bytes ELSE 0 END), 0) \
         FROM http_access_logs \
         INDEXED BY http_access_logs_gateway_auth_rejected_retention_idx",
    )
    .fetch_one(connection)
    .await?;
    Ok(CapacityStats {
        rows: to_u64(row.0)?,
        exchange_bytes: to_u64(row.1)?,
        gateway_auth_rejected_rows: to_u64(row.2)?,
        gateway_auth_rejected_exchange_bytes: to_u64(row.3)?,
    })
}

async fn gateway_auth_rejected_delete_count(
    connection: &mut SqliteConnection,
    rows_to_free: u64,
    bytes_to_free: u64,
) -> Result<u64, StorageError> {
    let delete_count: Option<i64> = sqlx::query_scalar(
        "SELECT delete_count FROM (\
             SELECT ROW_NUMBER() OVER (ORDER BY started_at_ms ASC, request_id ASC) \
                        AS delete_count, \
                    SUM(exchange_bytes) OVER (ORDER BY started_at_ms ASC, request_id ASC \
                        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS freed_bytes \
             FROM http_access_logs \
             INDEXED BY http_access_logs_gateway_auth_rejected_retention_idx \
             WHERE gateway_auth_rejected = 1\
         ) WHERE delete_count >= ? AND freed_bytes >= ? \
         ORDER BY delete_count ASC LIMIT 1",
    )
    .bind(to_i64(rows_to_free)?)
    .bind(to_i64(bytes_to_free)?)
    .fetch_optional(connection)
    .await?;
    delete_count
        .map(to_u64)
        .transpose()?
        .ok_or(StorageError::CorruptTelemetry)
}

async fn trim_global_capacity(
    connection: &mut SqliteConnection,
    capacity: HttpAccessLogCapacity,
    stats: CapacityStats,
) -> Result<u64, StorageError> {
    let excess_rows = stats.rows.saturating_sub(capacity.max_rows());
    let excess_bytes = stats
        .exchange_bytes
        .saturating_sub(capacity.max_exchange_bytes());
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
