use std::collections::HashMap;

use any2api_domain::GatewayApiKeyId;
use async_trait::async_trait;
use sqlx::{FromRow, SqliteConnection};

use crate::{
    error::StorageError,
    request_log::usage_window::{
        REQUEST_USAGE_WINDOW_MS, RequestUsageWindowSlot, build_request_usage_window_slots,
        request_usage_window_range_start, unix_now_ms,
    },
    sqlite::SqliteStore,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyUsageSummary {
    pub id: GatewayApiKeyId,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub window_slots: Vec<RequestUsageWindowSlot>,
}

impl GatewayApiKeyUsageSummary {
    #[must_use]
    pub fn failed_requests(&self) -> u64 {
        self.total_requests.saturating_sub(self.successful_requests)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyLastUsedUpdate {
    pub id: GatewayApiKeyId,
    pub last_used_at: String,
}

#[async_trait]
pub trait GatewayApiKeyUsageRepository: Send + Sync {
    async fn touch_gateway_api_key_last_used(
        &self,
        updates: &[GatewayApiKeyLastUsedUpdate],
    ) -> Result<(), StorageError>;

    async fn list_gateway_api_key_usage(
        &self,
    ) -> Result<Vec<GatewayApiKeyUsageSummary>, StorageError>;
}

#[async_trait]
impl GatewayApiKeyUsageRepository for SqliteStore {
    async fn touch_gateway_api_key_last_used(
        &self,
        updates: &[GatewayApiKeyLastUsedUpdate],
    ) -> Result<(), StorageError> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for update in updates {
            touch_one(&mut transaction, update).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn list_gateway_api_key_usage(
        &self,
    ) -> Result<Vec<GatewayApiKeyUsageSummary>, StorageError> {
        let now_ms = unix_now_ms()?;
        let range_start_ms = request_usage_window_range_start(now_ms);
        let mut transaction = self.pool().begin().await?;
        let summary_rows = sqlx::query_as::<_, GatewayApiKeyUsageRow>(
            "SELECT gateway_api_key_id, COUNT(*) AS total_requests, \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
             AS successful_requests \
             FROM request_logs \
             WHERE gateway_api_key_id IS NOT NULL \
             GROUP BY gateway_api_key_id",
        )
        .fetch_all(&mut *transaction)
        .await?;
        let slot_rows = sqlx::query_as::<_, GatewayApiKeyWindowSlotRow>(
            "SELECT gateway_api_key_id, \
             (started_at_ms / ?) * ? AS bucket_start_ms, \
             COUNT(*) AS total_requests, \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
             AS successful_requests \
             FROM request_logs \
             WHERE gateway_api_key_id IS NOT NULL AND started_at_ms >= ? \
             GROUP BY gateway_api_key_id, bucket_start_ms",
        )
        .bind(i64::try_from(REQUEST_USAGE_WINDOW_MS).map_err(|_| StorageError::CorruptTelemetry)?)
        .bind(i64::try_from(REQUEST_USAGE_WINDOW_MS).map_err(|_| StorageError::CorruptTelemetry)?)
        .bind(i64::try_from(range_start_ms).map_err(|_| StorageError::CorruptTelemetry)?)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let mut slots_by_id: HashMap<GatewayApiKeyId, HashMap<u64, (u64, u64)>> = HashMap::new();
        for row in slot_rows {
            let id = parse_id(&row.gateway_api_key_id)?;
            let bucket = from_i64(row.bucket_start_ms)?;
            let total = from_i64(row.total_requests)?;
            let successful = from_i64(row.successful_requests)?;
            if successful > total {
                return Err(StorageError::CorruptTelemetry);
            }
            slots_by_id
                .entry(id)
                .or_default()
                .insert(bucket, (total, successful));
        }

        summary_rows
            .into_iter()
            .map(|row| parse_usage_row(row, now_ms, &mut slots_by_id))
            .collect()
    }
}

async fn touch_one(
    connection: &mut SqliteConnection,
    update: &GatewayApiKeyLastUsedUpdate,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE gateway_api_keys \
         SET last_used_at = ? \
         WHERE id = ? \
           AND (last_used_at IS NULL OR last_used_at < ?)",
    )
    .bind(&update.last_used_at)
    .bind(update.id.to_string())
    .bind(&update.last_used_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}

#[derive(FromRow)]
struct GatewayApiKeyUsageRow {
    gateway_api_key_id: String,
    total_requests: i64,
    successful_requests: i64,
}

#[derive(FromRow)]
struct GatewayApiKeyWindowSlotRow {
    gateway_api_key_id: String,
    bucket_start_ms: i64,
    total_requests: i64,
    successful_requests: i64,
}

fn parse_usage_row(
    row: GatewayApiKeyUsageRow,
    now_ms: u64,
    slots_by_id: &mut HashMap<GatewayApiKeyId, HashMap<u64, (u64, u64)>>,
) -> Result<GatewayApiKeyUsageSummary, StorageError> {
    let total_requests = from_i64(row.total_requests)?;
    let successful_requests = from_i64(row.successful_requests)?;
    if successful_requests > total_requests {
        return Err(StorageError::CorruptTelemetry);
    }
    let id = parse_id(&row.gateway_api_key_id)?;
    let filled = slots_by_id.remove(&id).unwrap_or_default();
    Ok(GatewayApiKeyUsageSummary {
        id,
        total_requests,
        successful_requests,
        window_slots: build_request_usage_window_slots(now_ms, &filled),
    })
}

fn parse_id<T: std::str::FromStr>(value: &str) -> Result<T, StorageError> {
    value.parse().map_err(|_| StorageError::CorruptTelemetry)
}

fn from_i64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
