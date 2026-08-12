use any2api_domain::{LogPage, LogPageCursor, LogPagePosition, RequestLog};

use crate::{error::StorageError, sqlite::SqliteStore};

use super::rows::{RequestLogRow, parse_request_log};

const REQUEST_LOG_PAGE_COLUMNS: &str = "request_id, started_at_ms, client_ip, config_revision, \
    gateway_api_key_id, ingress_protocol, operation, public_model, thinking_level, \
    provider_endpoint_id, credential_id, oauth_account_id, proxy_profile_id, status_code, \
    error_class, error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
    output_tokens, cache_read_tokens, quota_cost_unit, quota_cost_nanos, quota_cost_rate_card, \
    quota_service_tier, telemetry_process_id, telemetry_sequence, is_stream";

pub(super) async fn list(
    store: &SqliteStore,
    since_ms: u64,
    cursor: Option<LogPageCursor>,
    limit: u32,
) -> Result<LogPage<RequestLog>, StorageError> {
    let since_ms = to_i64(since_ms)?;
    let mut transaction = store.pool().begin().await?;
    let cursor = match cursor {
        Some(cursor) => cursor,
        None => {
            let row = sqlx::query_as::<_, (i64, String)>(
                "SELECT started_at_ms, request_id FROM request_logs \
                 WHERE started_at_ms >= ? ORDER BY started_at_ms DESC, request_id DESC LIMIT 1",
            )
            .bind(since_ms)
            .fetch_optional(&mut *transaction)
            .await?;
            let Some((started_at_ms, request_id)) = row else {
                transaction.commit().await?;
                return Ok(LogPage::empty());
            };
            LogPageCursor::first(LogPagePosition::new(to_u64(started_at_ms)?, request_id))
        }
    };

    let anchor = cursor.anchor();
    let anchor_started_at_ms = to_i64(anchor.started_at_ms())?;
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM request_logs WHERE started_at_ms >= ? \
         AND (started_at_ms, request_id) <= (?, ?)",
    )
    .bind(since_ms)
    .bind(anchor_started_at_ms)
    .bind(anchor.request_id())
    .fetch_one(&mut *transaction)
    .await?;

    let fetch_limit = i64::from(limit) + 1;
    let mut rows = match cursor.before() {
        Some(before) => {
            let statement = format!(
                "SELECT {REQUEST_LOG_PAGE_COLUMNS} FROM request_logs WHERE started_at_ms >= ? \
                 AND (started_at_ms, request_id) <= (?, ?) \
                 AND (started_at_ms, request_id) < (?, ?) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT ?"
            );
            sqlx::query_as::<_, RequestLogRow>(&statement)
                .bind(since_ms)
                .bind(anchor_started_at_ms)
                .bind(anchor.request_id())
                .bind(to_i64(before.started_at_ms())?)
                .bind(before.request_id())
                .bind(fetch_limit)
                .fetch_all(&mut *transaction)
                .await?
        }
        None => {
            let statement = format!(
                "SELECT {REQUEST_LOG_PAGE_COLUMNS} FROM request_logs WHERE started_at_ms >= ? \
                 AND (started_at_ms, request_id) <= (?, ?) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT ?"
            );
            sqlx::query_as::<_, RequestLogRow>(&statement)
                .bind(since_ms)
                .bind(anchor_started_at_ms)
                .bind(anchor.request_id())
                .bind(fetch_limit)
                .fetch_all(&mut *transaction)
                .await?
        }
    };
    transaction.commit().await?;

    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if has_more {
        let boundary = rows
            .last()
            .ok_or(StorageError::CorruptTelemetry)?
            .page_position()?;
        Some(
            LogPageCursor::next(cursor.anchor().clone(), boundary)
                .ok_or(StorageError::CorruptTelemetry)?,
        )
    } else {
        None
    };
    let (items, corrupt_rows) = parse_page_rows(rows)?;
    if corrupt_rows > 0 {
        tracing::warn!(corrupt_rows, "corrupt request telemetry rows were skipped");
    }
    Ok(LogPage::new(
        items,
        to_u64(total)?,
        Some(cursor),
        next_cursor,
    ))
}

fn parse_page_rows(rows: Vec<RequestLogRow>) -> Result<(Vec<RequestLog>, usize), StorageError> {
    let mut items = Vec::with_capacity(rows.len());
    let mut corrupt_rows = 0;
    for row in rows {
        match parse_request_log(row) {
            Ok(log) => items.push(log),
            Err(StorageError::CorruptTelemetry) => corrupt_rows += 1,
            Err(error) => return Err(error),
        }
    }
    Ok((items, corrupt_rows))
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
