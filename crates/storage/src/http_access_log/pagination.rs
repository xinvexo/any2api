use any2api_domain::{HttpAccessLogSummary, LogPage, LogPageCursor, LogPagePosition};

use crate::{error::StorageError, sqlite::SqliteStore};

use super::rows::{HttpAccessLogSummaryRow, parse_summary};

pub(crate) const SYSTEM_LOG_RETENTION_PREDICATE: &str = "\
    path = '/v1' OR path GLOB '/v1/*' OR client_ip IS NULL OR \
    (client_ip NOT LIKE '127.%' AND client_ip <> '::1') OR \
    status_code IS NULL OR status_code >= 400 OR outcome <> 'completed'";

const SYSTEM_LOG_PAGE_COLUMNS: &str = "request_id, started_at_ms, config_revision, client_ip, \
    method, path, uri, http_version, status_code, duration_ms, response_bytes, outcome, \
    exchange_captured";

pub(super) async fn list(
    store: &SqliteStore,
    since_ms: u64,
    cursor: Option<LogPageCursor>,
    requested_page: u32,
    limit: u32,
) -> Result<LogPage<HttpAccessLogSummary>, StorageError> {
    let since_ms = to_i64(since_ms)?;
    let mut transaction = store.pool().begin().await?;
    let requested_cursor = match cursor {
        Some(cursor) => cursor,
        None => {
            let statement = format!(
                "SELECT started_at_ms, request_id FROM http_access_logs \
                 INDEXED BY http_access_logs_summary_filter_idx \
                 WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT 1"
            );
            let row = sqlx::query_as::<_, (i64, String)>(&statement)
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

    let anchor = requested_cursor.anchor().clone();
    let anchor_started_at_ms = to_i64(anchor.started_at_ms())?;
    let count_statement = format!(
        "SELECT COUNT(*) FROM http_access_logs \
         INDEXED BY http_access_logs_summary_filter_idx \
         WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
         AND (started_at_ms, request_id) <= (?, ?)"
    );
    let total: i64 = sqlx::query_scalar(&count_statement)
        .bind(since_ms)
        .bind(anchor_started_at_ms)
        .bind(anchor.request_id())
        .fetch_one(&mut *transaction)
        .await?;
    let total = to_u64(total)?;
    let page = clamp_page(requested_page, total, limit);
    let cursor = if page == 1 {
        LogPageCursor::first(anchor.clone())
    } else if page == requested_page && requested_cursor.before().is_some() {
        requested_cursor
    } else {
        let boundary_statement = format!(
            "SELECT started_at_ms, request_id FROM http_access_logs \
             INDEXED BY http_access_logs_summary_filter_idx \
             WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
             AND (started_at_ms, request_id) <= (?, ?) \
             ORDER BY started_at_ms DESC, request_id DESC LIMIT 1 OFFSET ?"
        );
        let (started_at_ms, request_id) = sqlx::query_as::<_, (i64, String)>(&boundary_statement)
            .bind(since_ms)
            .bind(anchor_started_at_ms)
            .bind(anchor.request_id())
            .bind(page_boundary_offset(page, limit)?)
            .fetch_one(&mut *transaction)
            .await?;
        LogPageCursor::next(
            anchor.clone(),
            LogPagePosition::new(to_u64(started_at_ms)?, request_id),
        )
        .ok_or(StorageError::CorruptTelemetry)?
    };

    let fetch_limit = i64::from(limit) + 1;
    let mut rows = match cursor.before() {
        Some(before) => {
            let statement = format!(
                "SELECT {SYSTEM_LOG_PAGE_COLUMNS} FROM http_access_logs \
                 INDEXED BY http_access_logs_summary_filter_idx \
                 WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
                 AND (started_at_ms, request_id) <= (?, ?) \
                 AND (started_at_ms, request_id) < (?, ?) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT ?"
            );
            sqlx::query_as::<_, HttpAccessLogSummaryRow>(&statement)
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
                "SELECT {SYSTEM_LOG_PAGE_COLUMNS} FROM http_access_logs \
                 INDEXED BY http_access_logs_summary_filter_idx \
                 WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
                 AND (started_at_ms, request_id) <= (?, ?) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT ?"
            );
            sqlx::query_as::<_, HttpAccessLogSummaryRow>(&statement)
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
    let items = rows
        .into_iter()
        .map(parse_summary)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LogPage::new(items, total, page, Some(cursor), next_cursor))
}

fn clamp_page(requested_page: u32, total: u64, limit: u32) -> u32 {
    let total_pages = total.div_ceil(u64::from(limit)).max(1);
    u64::from(requested_page.max(1)).min(total_pages) as u32
}

fn page_boundary_offset(page: u32, limit: u32) -> Result<i64, StorageError> {
    let offset = (u64::from(page) - 1)
        .saturating_mul(u64::from(limit))
        .saturating_sub(1);
    i64::try_from(offset).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
