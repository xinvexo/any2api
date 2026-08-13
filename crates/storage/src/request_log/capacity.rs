use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::writes::delete_oldest;

pub const REQUEST_LOG_CLEANUP_BATCH_ROWS: u32 = 10_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RequestLogCleanupOutcome {
    deleted_rows: u64,
    has_more: bool,
}

impl RequestLogCleanupOutcome {
    #[must_use]
    pub const fn new(deleted_rows: u64, has_more: bool) -> Self {
        Self {
            deleted_rows,
            has_more,
        }
    }

    #[must_use]
    pub const fn deleted_rows(&self) -> u64 {
        self.deleted_rows
    }

    #[must_use]
    pub const fn has_more(&self) -> bool {
        self.has_more
    }
}

pub(super) async fn trim_to_capacity(
    connection: &mut SqliteConnection,
    max_rows: u64,
    delete_budget: u64,
) -> Result<RequestLogCleanupOutcome, StorageError> {
    if delete_budget == 0 {
        return Ok(RequestLogCleanupOutcome::default());
    }
    let excess = stored_rows(connection).await?.saturating_sub(max_rows);
    if excess == 0 {
        return Ok(RequestLogCleanupOutcome::default());
    }
    let deleted_rows = delete_oldest(connection, excess.min(delete_budget)).await?;
    Ok(RequestLogCleanupOutcome::new(
        deleted_rows,
        deleted_rows == delete_budget,
    ))
}

async fn stored_rows(connection: &mut SqliteConnection) -> Result<u64, StorageError> {
    // Maintained incrementally by the request_logs triggers from migration
    // 0015 so the common under-capacity case never touches the log table.
    let rows: Option<i64> = sqlx::query_scalar(
        "SELECT request_log_rows FROM telemetry_capacity_stats \
             WHERE singleton_id = 1",
    )
    .fetch_optional(connection)
    .await?;
    rows.map(to_u64)
        .transpose()?
        .ok_or(StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
