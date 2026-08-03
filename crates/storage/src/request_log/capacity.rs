use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::writes::delete_oldest;

pub const REQUEST_LOG_CLEANUP_BATCH_ROWS: u32 = 10_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
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
    pub const fn deleted_rows(self) -> u64 {
        self.deleted_rows
    }

    #[must_use]
    pub const fn has_more(self) -> bool {
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
    if max_rows == 0 {
        let deleted_rows = delete_oldest(connection, delete_budget).await?;
        return Ok(RequestLogCleanupOutcome::new(
            deleted_rows,
            deleted_rows == delete_budget,
        ));
    }

    let boundary = sqlx::query_as::<_, (i64, String)>(
        "SELECT started_at_ms, request_id FROM request_logs \
         INDEXED BY request_logs_started_idx \
         ORDER BY started_at_ms DESC, request_id DESC LIMIT 1 OFFSET ?",
    )
    .bind(to_i64(max_rows - 1)?)
    .fetch_optional(&mut *connection)
    .await?;
    let Some((started_at_ms, request_id)) = boundary else {
        return Ok(RequestLogCleanupOutcome::default());
    };
    let result = sqlx::query(
        "DELETE FROM request_logs WHERE request_id IN (\
             SELECT request_id FROM request_logs INDEXED BY request_logs_started_idx \
             WHERE started_at_ms < ? OR (started_at_ms = ? AND request_id < ?) \
             ORDER BY started_at_ms ASC, request_id ASC LIMIT ?\
         )",
    )
    .bind(started_at_ms)
    .bind(started_at_ms)
    .bind(request_id)
    .bind(to_i64(delete_budget)?)
    .execute(connection)
    .await?;
    let deleted_rows = result.rows_affected();
    Ok(RequestLogCleanupOutcome::new(
        deleted_rows,
        deleted_rows == delete_budget,
    ))
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
