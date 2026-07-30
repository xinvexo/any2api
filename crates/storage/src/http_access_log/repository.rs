use any2api_domain::{HttpAccessLog, LogPage};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    rows::{HttpAccessLogRow, parse},
    writes::{delete_oldest, delete_oldest_before, insert},
};

#[async_trait]
pub trait HttpAccessLogRepository: Send + Sync {
    async fn append_http_access_logs(&self, records: &[HttpAccessLog]) -> Result<(), StorageError>;

    async fn prune_http_access_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<u64, StorageError>;

    async fn list_http_access_logs(
        &self,
        since_ms: u64,
        offset: u64,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLog>, StorageError>;

    async fn clear_http_access_logs(&self) -> Result<u64, StorageError>;
}

#[async_trait]
impl HttpAccessLogRepository for SqliteStore {
    async fn append_http_access_logs(&self, records: &[HttpAccessLog]) -> Result<(), StorageError> {
        if records.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for record in records {
            insert(&mut transaction, record).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn prune_http_access_logs(
        &self,
        retention_before_ms: u64,
        max_rows: u64,
        batch_size: u32,
    ) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let expired =
            delete_oldest_before(&mut transaction, retention_before_ms, u64::from(batch_size))
                .await?;
        transaction.commit().await?;
        // The row-cap count runs outside the write transaction so the periodic
        // full-table scan never blocks concurrent telemetry writers; a stale
        // count only shifts overflow trimming to the next prune cycle.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM http_access_logs")
            .fetch_one(self.pool())
            .await?;
        let count = u64::try_from(count).map_err(|_| StorageError::CorruptTelemetry)?;
        let overflow = count.saturating_sub(max_rows).min(u64::from(batch_size));
        let trimmed = if overflow == 0 {
            0
        } else {
            let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
            let trimmed = delete_oldest(&mut transaction, overflow).await?;
            transaction.commit().await?;
            trimmed
        };
        Ok(expired.saturating_add(trimmed))
    }

    async fn list_http_access_logs(
        &self,
        since_ms: u64,
        offset: u64,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLog>, StorageError> {
        let since_ms = to_i64(since_ms)?;
        let offset = to_i64(offset)?;
        let mut transaction = self.pool().begin().await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM http_access_logs WHERE started_at_ms >= ? AND (\
             path = '/v1' OR path GLOB '/v1/*' OR client_ip IS NULL OR \
             (client_ip NOT LIKE '127.%' AND client_ip <> '::1') OR \
             status_code IS NULL OR status_code >= 400 OR outcome <> 'completed')",
        )
        .bind(since_ms)
        .fetch_one(&mut *transaction)
        .await?;
        let rows = sqlx::query_as::<_, HttpAccessLogRow>(
            "SELECT request_id, started_at_ms, config_revision, client_ip, method, path, \
             http_version, status_code, duration_ms, response_bytes, outcome \
             FROM http_access_logs WHERE started_at_ms >= ? AND (\
             path = '/v1' OR path GLOB '/v1/*' OR client_ip IS NULL OR \
             (client_ip NOT LIKE '127.%' AND client_ip <> '::1') OR \
             status_code IS NULL OR status_code >= 400 OR outcome <> 'completed') \
             ORDER BY started_at_ms DESC, request_id DESC LIMIT ? OFFSET ?",
        )
        .bind(since_ms)
        .bind(i64::from(limit))
        .bind(offset)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;
        let items = rows.into_iter().map(parse).collect::<Result<Vec<_>, _>>()?;
        Ok(LogPage::new(
            items,
            u64::try_from(total).map_err(|_| StorageError::CorruptTelemetry)?,
        ))
    }

    async fn clear_http_access_logs(&self) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM http_access_logs")
            .execute(self.pool())
            .await?;
        Ok(result.rows_affected())
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
