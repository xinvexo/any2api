use any2api_domain::{OAuthAccountId, QuotaCostUnit, RequestTelemetryPosition};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

#[async_trait]
pub trait OAuthQuotaEstimationRepository: Send + Sync {
    async fn oauth_quota_window_local_cost_nanos(
        &self,
        id: OAuthAccountId,
        window_started_at_ms: u64,
        observed_at_ms: u64,
        observation_position: RequestTelemetryPosition,
        unit: QuotaCostUnit,
    ) -> Result<u64, StorageError>;
}

#[async_trait]
impl OAuthQuotaEstimationRepository for SqliteStore {
    async fn oauth_quota_window_local_cost_nanos(
        &self,
        id: OAuthAccountId,
        window_started_at_ms: u64,
        observed_at_ms: u64,
        observation_position: RequestTelemetryPosition,
        unit: QuotaCostUnit,
    ) -> Result<u64, StorageError> {
        if window_started_at_ms > observed_at_ms {
            return Err(StorageError::CorruptTelemetry);
        }
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(quota_cost_nanos), 0) \
             FROM request_logs \
             WHERE oauth_account_id = ? AND started_at_ms <= ? \
             AND (started_at_ms + latency_ms) >= ? \
             AND (started_at_ms + latency_ms) <= ? \
             AND quota_cost_unit = ? \
             AND (telemetry_process_id IS NULL OR telemetry_process_id <> ? \
                  OR telemetry_sequence <= ?)",
        )
        .bind(id.to_string())
        .bind(to_i64(observed_at_ms)?)
        .bind(to_i64(window_started_at_ms)?)
        .bind(to_i64(observed_at_ms)?)
        .bind(unit.as_str())
        .bind(observation_position.process_id.to_string())
        .bind(to_i64(observation_position.sequence)?)
        .fetch_one(self.pool())
        .await?;
        u64::try_from(total).map_err(|_| StorageError::CorruptTelemetry)
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
