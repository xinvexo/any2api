use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::{
    HttpAccessLogCapacity,
    writes::{delete_oldest, delete_oldest_gateway_auth_rejected},
};

#[derive(Clone, Copy)]
struct CapacityStats {
    rows: u64,
    gateway_auth_rejected_rows: u64,
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
    let mut deleted = 0;
    if rejected_rows_to_free > 0 {
        deleted = delete_oldest_gateway_auth_rejected(connection, rejected_rows_to_free).await?;
    }
    let remaining = if deleted == 0 {
        stats
    } else {
        capacity_stats(connection).await?
    };
    Ok(deleted.saturating_add(trim_global_capacity(connection, capacity, remaining).await?))
}

async fn capacity_stats(connection: &mut SqliteConnection) -> Result<CapacityStats, StorageError> {
    // Maintained incrementally by the http_access_logs triggers from migration
    // 0015 so trim decisions never rescan the table under the write lock.
    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT http_access_log_rows, gateway_auth_rejected_rows \
         FROM telemetry_capacity_stats WHERE singleton_id = 1",
    )
    .fetch_optional(connection)
    .await?
    .ok_or(StorageError::CorruptTelemetry)?;
    Ok(CapacityStats {
        rows: to_u64(row.0)?,
        gateway_auth_rejected_rows: to_u64(row.1)?,
    })
}

async fn trim_global_capacity(
    connection: &mut SqliteConnection,
    capacity: HttpAccessLogCapacity,
    stats: CapacityStats,
) -> Result<u64, StorageError> {
    let excess_rows = stats.rows.saturating_sub(capacity.max_rows());
    if excess_rows == 0 {
        return Ok(0);
    }
    delete_oldest(connection, excess_rows).await
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
