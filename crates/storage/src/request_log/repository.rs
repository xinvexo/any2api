use any2api_domain::{CompletedRequestLog, LogPage, LogPageCursor, RequestId, RequestLog};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    capacity::{REQUEST_LOG_CLEANUP_BATCH_ROWS, RequestLogCleanupOutcome, trim_to_capacity},
    overview::{RequestLogOverview, RequestLogOverviewRange, load_request_log_overview},
    pagination,
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
        cursor: Option<LogPageCursor>,
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
        let mut transaction = self.begin_write().await?;
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
        let mut transaction = self.begin_write().await?;
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
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<RequestLog>, StorageError> {
        pagination::list(self, since_ms, cursor, limit).await
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
             proxy_profile_id, routing_mode, started_at_ms, duration_ms, retry_safety, \
             failure_scope, retry_decision, error_class, error_message, status_code, outcome, \
             transport_wire_profile_id, transport_wire_profile_version, \
             transport_timeout_policy_version, transport_resolver_mode, transport_proxy_kind, \
             transport_connect_timeout_ms, transport_read_timeout_ms, \
             transport_pool_idle_timeout_ms, transport_routing_generation, \
             transport_authentication_version, transport_traffic_class, \
             first_upstream_frame_ms, stream_commit_ms, first_downstream_byte_ms, stream_cancel_ms \
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
