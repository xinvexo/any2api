use any2api_domain::{HttpAccessLogSummary, LogBatch, LogCursor, LogCursorPosition};
use sqlx::AssertSqlSafe;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::rows::{HttpAccessLogSummaryRow, parse_summary};

pub(crate) const SYSTEM_LOG_RETENTION_PREDICATE: &str = "\
    path = '/v1' OR path GLOB '/v1/*' OR client_ip IS NULL OR \
    (client_ip NOT LIKE '127.%' AND client_ip <> '::1') OR \
    status_code IS NULL OR status_code >= 400 OR outcome <> 'completed'";

pub(crate) const HIDE_ADMIN_OPERATIONS_PREDICATE: &str = "\
    path <> '/api/admin' AND path NOT GLOB '/api/admin/*' AND \
    path NOT GLOB '/assets/*' AND path NOT IN (\
        '/any2api-icon.png', '/boot-theme.js', '/favicon-16x16.png', \
        '/favicon-32x32.png', '/apple-touch-icon.png', '/index.html'\
    )";

pub(super) const SYSTEM_LOG_BATCH_COLUMNS: &str = "request_id, started_at_ms, config_revision, client_ip, \
    method, path, uri, http_version, status_code, duration_ms, response_bytes, outcome, \
    exchange_captured";

pub(super) async fn list(
    store: &SqliteStore,
    since_ms: u64,
    show_admin_operations: bool,
    cursor: Option<LogCursor>,
    limit: u32,
) -> Result<LogBatch<HttpAccessLogSummary>, StorageError> {
    let since_ms = to_i64(since_ms)?;
    let admin_operations_predicate = if show_admin_operations {
        "TRUE"
    } else {
        HIDE_ADMIN_OPERATIONS_PREDICATE
    };
    let mut transaction = store.pool().begin().await?;
    let requested_cursor = match cursor {
        Some(cursor) => cursor,
        None => {
            let statement = format!(
                "SELECT started_at_ms, request_id FROM http_access_logs \
                 INDEXED BY http_access_logs_summary_filter_idx \
                 WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
                 AND ({admin_operations_predicate}) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT 1"
            );
            let row = sqlx::query_as::<_, (i64, String)>(AssertSqlSafe(statement))
                .bind(since_ms)
                .fetch_optional(&mut *transaction)
                .await?;
            let Some((started_at_ms, request_id)) = row else {
                transaction.commit().await?;
                return Ok(LogBatch::empty());
            };
            LogCursor::first(LogCursorPosition::new(to_u64(started_at_ms)?, request_id))
        }
    };

    let cursor = requested_cursor;
    let anchor = cursor.anchor().clone();
    let anchor_started_at_ms = to_i64(anchor.started_at_ms())?;

    let fetch_limit = i64::from(limit) + 1;
    let mut rows = match cursor.before() {
        Some(before) => {
            let statement = format!(
                "SELECT {SYSTEM_LOG_BATCH_COLUMNS} FROM http_access_logs \
                 INDEXED BY http_access_logs_summary_filter_idx \
                 WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
                 AND ({admin_operations_predicate}) \
                 AND (started_at_ms, request_id) <= (?, ?) \
                 AND (started_at_ms, request_id) < (?, ?) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT ?"
            );
            sqlx::query_as::<_, HttpAccessLogSummaryRow>(AssertSqlSafe(statement))
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
                "SELECT {SYSTEM_LOG_BATCH_COLUMNS} FROM http_access_logs \
                 INDEXED BY http_access_logs_summary_filter_idx \
                 WHERE started_at_ms >= ? AND ({SYSTEM_LOG_RETENTION_PREDICATE}) \
                 AND ({admin_operations_predicate}) \
                 AND (started_at_ms, request_id) <= (?, ?) \
                 ORDER BY started_at_ms DESC, request_id DESC LIMIT ?"
            );
            sqlx::query_as::<_, HttpAccessLogSummaryRow>(AssertSqlSafe(statement))
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
            .cursor_position()?;
        Some(
            LogCursor::next(cursor.anchor().clone(), boundary)
                .ok_or(StorageError::CorruptTelemetry)?,
        )
    } else {
        None
    };
    let items = rows
        .into_iter()
        .map(parse_summary)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LogBatch::new(items, next_cursor))
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
