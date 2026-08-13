use any2api_domain::{OAuthAccountId, QuotaCostUnit, RequestTelemetryPosition};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

#[async_trait]
pub trait OAuthQuotaEstimationRepository: Send + Sync {
    async fn oauth_quota_local_cost_nanos(
        &self,
        id: OAuthAccountId,
        interval_start: RequestTelemetryPosition,
        interval_end: RequestTelemetryPosition,
        unit: QuotaCostUnit,
    ) -> Result<u64, StorageError>;
}

#[async_trait]
impl OAuthQuotaEstimationRepository for SqliteStore {
    async fn oauth_quota_local_cost_nanos(
        &self,
        id: OAuthAccountId,
        interval_start: RequestTelemetryPosition,
        interval_end: RequestTelemetryPosition,
        unit: QuotaCostUnit,
    ) -> Result<u64, StorageError> {
        if interval_start.process_id != interval_end.process_id
            || interval_start.sequence > interval_end.sequence
        {
            return Err(StorageError::CorruptTelemetry);
        }
        if interval_start.sequence == interval_end.sequence {
            return Ok(0);
        }
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(quota_cost_nanos), 0) \
             FROM request_logs \
             WHERE oauth_account_id = ? AND telemetry_process_id = ? \
             AND telemetry_sequence > ? AND telemetry_sequence <= ? \
             AND quota_cost_unit = ?",
        )
        .bind(id.to_string())
        .bind(interval_start.process_id.to_string())
        .bind(to_i64(interval_start.sequence)?)
        .bind(to_i64(interval_end.sequence)?)
        .bind(unit.as_str())
        .fetch_one(self.pool())
        .await?;
        u64::try_from(total).map_err(|_| StorageError::CorruptTelemetry)
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
