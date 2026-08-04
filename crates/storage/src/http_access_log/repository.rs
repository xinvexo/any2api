use any2api_domain::{
    HttpAccessLog, HttpAccessLogSummary, LogPage, LogPageCursor, RequestId,
    gateway_auth_rejected_capacity,
};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

use super::{
    capacity::trim_to_capacity,
    pagination,
    rows::{HttpAccessLogDetailRow, parse_detail},
    writes::{delete_oldest_before, insert},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpAccessLogCapacity {
    max_rows: u64,
    max_exchange_bytes: u64,
}

impl HttpAccessLogCapacity {
    #[must_use]
    pub const fn new(max_rows: u64, max_exchange_bytes: u64) -> Self {
        Self {
            max_rows,
            max_exchange_bytes,
        }
    }

    pub const fn max_rows(self) -> u64 {
        self.max_rows
    }

    pub const fn max_exchange_bytes(self) -> u64 {
        self.max_exchange_bytes
    }

    pub const fn gateway_auth_rejected_max_rows(self) -> u64 {
        gateway_auth_rejected_capacity(self.max_rows)
    }

    pub const fn gateway_auth_rejected_max_exchange_bytes(self) -> u64 {
        gateway_auth_rejected_capacity(self.max_exchange_bytes)
    }
}

#[async_trait]
pub trait HttpAccessLogRepository: Send + Sync {
    async fn append_http_access_logs(
        &self,
        records: &[HttpAccessLog],
        capacity: HttpAccessLogCapacity,
    ) -> Result<u64, StorageError>;

    async fn prune_http_access_logs(
        &self,
        retention_before_ms: u64,
        capacity: HttpAccessLogCapacity,
        batch_size: u32,
    ) -> Result<u64, StorageError>;

    async fn reclaim_http_access_log_storage(&self, max_bytes: u64) -> Result<u64, StorageError>;

    async fn list_http_access_logs(
        &self,
        since_ms: u64,
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLogSummary>, StorageError>;

    async fn get_http_access_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<HttpAccessLog>, StorageError>;

    async fn clear_http_access_logs(&self) -> Result<u64, StorageError>;
}

#[async_trait]
impl HttpAccessLogRepository for SqliteStore {
    async fn append_http_access_logs(
        &self,
        records: &[HttpAccessLog],
        capacity: HttpAccessLogCapacity,
    ) -> Result<u64, StorageError> {
        if records.is_empty() {
            return Ok(0);
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for record in records {
            insert(&mut transaction, record).await?;
        }
        let deleted = trim_to_capacity(&mut transaction, capacity).await?;
        transaction.commit().await?;
        Ok(deleted)
    }

    async fn prune_http_access_logs(
        &self,
        retention_before_ms: u64,
        capacity: HttpAccessLogCapacity,
        batch_size: u32,
    ) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let expired =
            delete_oldest_before(&mut transaction, retention_before_ms, u64::from(batch_size))
                .await?;
        let trimmed = trim_to_capacity(&mut transaction, capacity).await?;
        transaction.commit().await?;
        Ok(expired.saturating_add(trimmed))
    }

    async fn reclaim_http_access_log_storage(&self, max_bytes: u64) -> Result<u64, StorageError> {
        let mut connection = self.pool().acquire().await?;
        let mode: i64 = sqlx::query_scalar("PRAGMA auto_vacuum")
            .fetch_one(&mut *connection)
            .await?;
        if mode != 2 {
            return Ok(0);
        }
        let freelist: i64 = sqlx::query_scalar("PRAGMA freelist_count")
            .fetch_one(&mut *connection)
            .await?;
        let freelist = u64::try_from(freelist).map_err(|_| StorageError::CorruptTelemetry)?;
        if freelist == 0 {
            return Ok(0);
        }
        let page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&mut *connection)
            .await?;
        let page_size = u64::try_from(page_size).map_err(|_| StorageError::CorruptTelemetry)?;
        if page_size == 0 {
            return Err(StorageError::CorruptTelemetry);
        }
        let pages = (max_bytes / page_size).max(1).min(freelist);
        let page_count_before: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&mut *connection)
            .await?;
        let statement = format!("PRAGMA incremental_vacuum({pages})");
        sqlx::query(&statement).fetch_all(&mut *connection).await?;
        let page_count_after: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&mut *connection)
            .await?;
        Ok(to_u64(page_count_before)?.saturating_sub(to_u64(page_count_after)?))
    }

    async fn list_http_access_logs(
        &self,
        since_ms: u64,
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLogSummary>, StorageError> {
        pagination::list(self, since_ms, cursor, limit).await
    }

    async fn get_http_access_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<HttpAccessLog>, StorageError> {
        let row = sqlx::query_as::<_, HttpAccessLogDetailRow>(
            "SELECT request_id, started_at_ms, config_revision, client_ip, method, path, uri, \
             http_version, status_code, duration_ms, response_bytes, outcome, exchange_captured, \
             gateway_auth_rejected, request_headers, request_body, request_body_bytes, request_body_complete, \
             request_body_truncated, response_headers, response_body, response_body_complete, \
             response_body_truncated FROM http_access_logs WHERE request_id = ?",
        )
        .bind(request_id.to_string())
        .fetch_optional(self.pool())
        .await?;
        row.map(parse_detail).transpose()
    }

    async fn clear_http_access_logs(&self) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let result = sqlx::query("DELETE FROM http_access_logs")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(result.rows_affected())
    }
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
