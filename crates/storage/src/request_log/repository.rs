use any2api_domain::{CompletedRequestLog, LogPage, RequestId, RequestLog};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    capacity::{REQUEST_LOG_CLEANUP_BATCH_ROWS, RequestLogCleanupOutcome, trim_to_capacity},
    overview::{RequestLogOverview, RequestLogOverviewRange, load_request_log_overview},
    rows::{RequestAttemptRow, RequestLogRow, parse_request_attempt, parse_request_log},
    writes::{delete_oldest_before, insert_request_attempt, insert_request_log},
};

#[async_trait]
pub trait RequestLogRepository: Send + Sync {
    async fn append_request_logs(
        &self,
        records: &[CompletedRequestLog],
        max_rows: u64,
    ) -> Result<RequestLogCleanupOutcome, StorageError>;

    async fn prune_request_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<RequestLogCleanupOutcome, StorageError>;

    async fn list_request_logs(
        &self,
        since_ms: u64,
        offset: u64,
        limit: u32,
    ) -> Result<LogPage<RequestLog>, StorageError>;

    async fn get_request_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError>;

    async fn request_log_overview(
        &self,
        range: RequestLogOverviewRange,
    ) -> Result<RequestLogOverview, StorageError>;
}

#[async_trait]
impl RequestLogRepository for SqliteStore {
    async fn append_request_logs(
        &self,
        records: &[CompletedRequestLog],
        max_rows: u64,
    ) -> Result<RequestLogCleanupOutcome, StorageError> {
        if records.is_empty() {
            return Ok(RequestLogCleanupOutcome::default());
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for record in records {
            insert_request_log(&mut transaction, &record.request).await?;
            for attempt in &record.attempts {
                insert_request_attempt(&mut transaction, attempt).await?;
            }
        }
        let cleanup = trim_to_capacity(
            &mut transaction,
            max_rows,
            u64::from(REQUEST_LOG_CLEANUP_BATCH_ROWS),
        )
        .await?;
        transaction.commit().await?;
        Ok(cleanup)
    }

    async fn prune_request_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<RequestLogCleanupOutcome, StorageError> {
        let delete_budget = u64::from(batch_size);
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let expired =
            delete_oldest_before(&mut transaction, retention_before_ms, delete_budget).await?;
        let capacity = trim_to_capacity(
            &mut transaction,
            max_rows,
            delete_budget.saturating_sub(expired),
        )
        .await?;
        transaction.commit().await?;
        Ok(RequestLogCleanupOutcome::new(
            expired.saturating_add(capacity.deleted_rows()),
            capacity.has_more() || (delete_budget > 0 && expired == delete_budget),
        ))
    }

    async fn list_request_logs(
        &self,
        since_ms: u64,
        offset: u64,
        limit: u32,
    ) -> Result<LogPage<RequestLog>, StorageError> {
        let since_ms = to_i64(since_ms)?;
        let offset = to_i64(offset)?;
        let mut transaction = self.pool().begin().await?;
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM request_logs WHERE started_at_ms >= ?")
                .bind(since_ms)
                .fetch_one(&mut *transaction)
                .await?;
        let rows = sqlx::query_as::<_, RequestLogRow>(
            "SELECT request_id, started_at_ms, client_ip, config_revision, gateway_api_key_id, \
             ingress_protocol, operation, public_model, thinking_level, provider_endpoint_id, \
             credential_id, oauth_account_id, proxy_profile_id, status_code, error_class, \
             error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
             output_tokens, cache_read_tokens, is_stream FROM request_logs \
             WHERE started_at_ms >= ? ORDER BY started_at_ms DESC, request_id DESC \
             LIMIT ? OFFSET ?",
        )
        .bind(since_ms)
        .bind(i64::from(limit))
        .bind(offset)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        let (items, corrupt_rows) = parse_page_rows(rows)?;
        if corrupt_rows > 0 {
            tracing::warn!(corrupt_rows, "corrupt request telemetry rows were skipped");
        }
        Ok(LogPage::new(
            items,
            u64::try_from(total).map_err(|_| StorageError::CorruptTelemetry)?,
        ))
    }

    async fn get_request_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let row = sqlx::query_as::<_, RequestLogRow>(
            "SELECT request_id, started_at_ms, client_ip, config_revision, gateway_api_key_id, \
             ingress_protocol, operation, public_model, thinking_level, provider_endpoint_id, \
             credential_id, oauth_account_id, proxy_profile_id, status_code, error_class, \
             error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
             output_tokens, cache_read_tokens, is_stream \
             FROM request_logs WHERE request_id = ?",
        )
        .bind(request_id.to_string())
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(row) = row else {
            transaction.commit().await?;
            return Ok(None);
        };
        let request = parse_request_log(row)?;
        let rows = sqlx::query_as::<_, RequestAttemptRow>(
            "SELECT request_id, attempt_no, route_target_id, credential_id, oauth_account_id, \
             proxy_profile_id, started_at_ms, duration_ms, retry_safety, error_class, \
             error_message, status_code, outcome \
             FROM request_attempts WHERE request_id = ? ORDER BY attempt_no ASC",
        )
        .bind(request_id.to_string())
        .fetch_all(&mut *transaction)
        .await?;
        let attempts = rows
            .into_iter()
            .map(parse_request_attempt)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await?;
        Ok(Some(CompletedRequestLog { request, attempts }))
    }

    async fn request_log_overview(
        &self,
        range: RequestLogOverviewRange,
    ) -> Result<RequestLogOverview, StorageError> {
        load_request_log_overview(self.pool(), range).await
    }
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
