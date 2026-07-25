use any2api_domain::{CompletedRequestLog, RequestId, RequestLog};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    rows::{RequestAttemptRow, RequestLogRow, parse_request_attempt, parse_request_log},
    writes::{delete_oldest, delete_oldest_before, insert_request_attempt, insert_request_log},
};

#[async_trait]
pub trait RequestLogRepository: Send + Sync {
    async fn append_request_logs(
        &self,
        records: &[CompletedRequestLog],
    ) -> Result<(), StorageError>;

    async fn prune_request_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<u64, StorageError>;

    async fn list_request_logs(&self, limit: u32) -> Result<Vec<RequestLog>, StorageError>;

    async fn get_request_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError>;
}

#[async_trait]
impl RequestLogRepository for SqliteStore {
    async fn append_request_logs(
        &self,
        records: &[CompletedRequestLog],
    ) -> Result<(), StorageError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for record in records {
            insert_request_log(&mut transaction, &record.request).await?;
            for attempt in &record.attempts {
                insert_request_attempt(&mut transaction, attempt).await?;
            }
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn prune_request_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let expired =
            delete_oldest_before(&mut transaction, retention_before_ms, u64::from(batch_size))
                .await?;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM request_logs")
            .fetch_one(&mut *transaction)
            .await?;
        let count = u64::try_from(count).map_err(|_| StorageError::CorruptTelemetry)?;
        let overflow = count.saturating_sub(max_rows).min(u64::from(batch_size));
        let trimmed = if overflow == 0 {
            0
        } else {
            delete_oldest(&mut transaction, overflow).await?
        };
        transaction.commit().await?;
        Ok(expired.saturating_add(trimmed))
    }

    async fn list_request_logs(&self, limit: u32) -> Result<Vec<RequestLog>, StorageError> {
        let rows = sqlx::query_as::<_, RequestLogRow>(
            "SELECT request_id, started_at_ms, client_ip, config_revision, gateway_api_key_id, \
             ingress_protocol, operation, public_model, thinking_level, provider_endpoint_id, \
             credential_id, oauth_account_id, proxy_profile_id, status_code, error_class, \
             error_message, attempt_count, latency_ms, first_token_ms, input_tokens, \
             output_tokens, cache_read_tokens, cache_write_tokens, is_stream FROM request_logs \
             ORDER BY started_at_ms DESC, request_id DESC LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(parse_request_log).collect()
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
             output_tokens, cache_read_tokens, cache_write_tokens, is_stream \
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
}
