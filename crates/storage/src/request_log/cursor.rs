use any2api_domain::{
    LogBatch, LogCursor, LogCursorPosition, RequestLog, RequestLogFilter, RequestLogOutcomeFilter,
};
use sqlx::{QueryBuilder, Sqlite};

use crate::{error::StorageError, sqlite::SqliteStore};

use super::rows::{RequestLogRow, parse_request_log};

const REQUEST_LOG_BATCH_COLUMNS: &str = "request_id, started_at_ms, client_ip, config_revision, \
    gateway_api_key_id, ingress_protocol, operation, public_model, thinking_level, \
    provider_endpoint_id, credential_id, oauth_account_id, proxy_profile_id, status_code, \
    error_class, error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
    output_tokens, cache_read_tokens, cache_creation_tokens, quota_cost_unit, quota_cost_nanos, quota_cost_rate_card, \
    quota_service_tier, telemetry_process_id, telemetry_sequence, is_stream";

pub(super) async fn list(
    store: &SqliteStore,
    since_ms: u64,
    filter: &RequestLogFilter,
    cursor: Option<LogCursor>,
    limit: u32,
) -> Result<LogBatch<RequestLog>, StorageError> {
    let since_ms = to_i64(since_ms)?;
    let mut transaction = store.pool().begin().await?;
    let requested_cursor = match cursor {
        Some(cursor) => cursor,
        None => {
            let mut query = QueryBuilder::<Sqlite>::new(
                "SELECT started_at_ms, request_id FROM request_logs WHERE ",
            );
            push_window_predicates(&mut query, since_ms, filter, None)?;
            query.push(" ORDER BY started_at_ms DESC, request_id DESC LIMIT 1");
            let row = query
                .build_query_as::<(i64, String)>()
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

    let fetch_limit = i64::from(limit) + 1;
    let mut batch_query = QueryBuilder::<Sqlite>::new(format!(
        "SELECT {REQUEST_LOG_BATCH_COLUMNS} FROM request_logs WHERE "
    ));
    push_window_predicates(&mut batch_query, since_ms, filter, Some(&anchor))?;
    if let Some(before) = cursor.before() {
        batch_query
            .push(" AND (started_at_ms, request_id) < (")
            .push_bind(to_i64(before.started_at_ms())?)
            .push(", ")
            .push_bind(before.request_id().to_owned())
            .push(")");
    }
    batch_query
        .push(" ORDER BY started_at_ms DESC, request_id DESC LIMIT ")
        .push_bind(fetch_limit);
    let mut rows = batch_query
        .build_query_as::<RequestLogRow>()
        .fetch_all(&mut *transaction)
        .await?;
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
    let (items, corrupt_rows) = parse_batch_rows(rows)?;
    if corrupt_rows > 0 {
        tracing::warn!(corrupt_rows, "corrupt request telemetry rows were skipped");
    }
    Ok(LogBatch::new(items, next_cursor))
}

fn push_window_predicates(
    query: &mut QueryBuilder<'_, Sqlite>,
    since_ms: i64,
    filter: &RequestLogFilter,
    anchor: Option<&LogCursorPosition>,
) -> Result<(), StorageError> {
    query.push("started_at_ms >= ").push_bind(since_ms);
    push_filter_predicates(query, filter);
    if let Some(anchor) = anchor {
        query
            .push(" AND (started_at_ms, request_id) <= (")
            .push_bind(to_i64(anchor.started_at_ms())?)
            .push(", ")
            .push_bind(anchor.request_id().to_owned())
            .push(")");
    }
    Ok(())
}

fn push_filter_predicates(query: &mut QueryBuilder<'_, Sqlite>, filter: &RequestLogFilter) {
    if let Some(outcome) = filter.outcome() {
        match outcome {
            RequestLogOutcomeFilter::Success => {
                query.push(" AND error_class IS NULL AND status_code >= 200 AND status_code < 300");
            }
            RequestLogOutcomeFilter::Failed => {
                query.push(
                    " AND (error_class IS NULL OR error_class <> 'cancelled') \
                     AND (error_class IS NOT NULL OR status_code < 200 OR status_code >= 300)",
                );
            }
            RequestLogOutcomeFilter::Cancelled => {
                query.push(" AND error_class = 'cancelled'");
            }
        }
    }
    if let Some(public_model) = filter.public_model() {
        query
            .push(" AND public_model = ")
            .push_bind(public_model.as_str().to_owned());
    }
    if let Some(id) = filter.gateway_api_key_id() {
        query
            .push(" AND gateway_api_key_id = ")
            .push_bind(id.to_string());
    }
}

fn parse_batch_rows(rows: Vec<RequestLogRow>) -> Result<(Vec<RequestLog>, usize), StorageError> {
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
