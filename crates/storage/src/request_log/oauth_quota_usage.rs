use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OAuthQuotaRequestLogUsage {
    pub unit: Option<QuotaCostUnit>,
    pub total_cost_nanos: u64,
    pub priced_request_count: u64,
    pub unpriced_request_count: u64,
    pub rate_cards: Vec<String>,
}

#[async_trait]
pub trait OAuthQuotaEstimationRepository: Send + Sync {
    async fn oauth_quota_request_log_usage(
        &self,
        id: OAuthAccountId,
        interval_started_at_ms: u64,
        interval_ended_at_ms: u64,
    ) -> Result<OAuthQuotaRequestLogUsage, StorageError>;
}

#[derive(sqlx::FromRow)]
struct UsageRow {
    quota_cost_unit: Option<String>,
    quota_cost_rate_card: Option<String>,
    request_count: i64,
    total_cost_nanos: i64,
}

#[async_trait]
impl OAuthQuotaEstimationRepository for SqliteStore {
    async fn oauth_quota_request_log_usage(
        &self,
        id: OAuthAccountId,
        interval_started_at_ms: u64,
        interval_ended_at_ms: u64,
    ) -> Result<OAuthQuotaRequestLogUsage, StorageError> {
        if interval_started_at_ms >= interval_ended_at_ms {
            return Ok(OAuthQuotaRequestLogUsage::default());
        }
        let rows = sqlx::query_as::<_, UsageRow>(
            "SELECT quota_cost_unit, quota_cost_rate_card, COUNT(*) AS request_count, \
             COALESCE(SUM(quota_cost_nanos), 0) AS total_cost_nanos \
             FROM request_logs \
             WHERE oauth_account_id = ? AND (started_at_ms + latency_ms) >= ? \
             AND (started_at_ms + latency_ms) < ? \
             GROUP BY quota_cost_unit, quota_cost_rate_card \
             ORDER BY quota_cost_unit ASC, quota_cost_rate_card ASC",
        )
        .bind(id.to_string())
        .bind(to_i64(interval_started_at_ms)?)
        .bind(to_i64(interval_ended_at_ms)?)
        .fetch_all(self.pool())
        .await?;

        fold_usage(rows)
    }
}

fn fold_usage(rows: Vec<UsageRow>) -> Result<OAuthQuotaRequestLogUsage, StorageError> {
    let mut usage = OAuthQuotaRequestLogUsage::default();
    for row in rows {
        let request_count = to_u64(row.request_count)?;
        let (Some(unit), Some(rate_card)) =
            (row.quota_cost_unit.as_deref(), row.quota_cost_rate_card)
        else {
            usage.unpriced_request_count =
                checked_add(usage.unpriced_request_count, request_count)?;
            continue;
        };
        let unit = QuotaCostUnit::parse(unit).ok_or(StorageError::CorruptTelemetry)?;
        if usage.unit.is_some_and(|observed| observed != unit) {
            return Err(StorageError::CorruptTelemetry);
        }
        usage.unit = Some(unit);
        usage.priced_request_count = checked_add(usage.priced_request_count, request_count)?;
        usage.total_cost_nanos =
            checked_add(usage.total_cost_nanos, to_u64(row.total_cost_nanos)?)?;
        usage.rate_cards.push(rate_card);
    }
    Ok(usage)
}

fn checked_add(left: u64, right: u64) -> Result<u64, StorageError> {
    left.checked_add(right)
        .ok_or(StorageError::CorruptTelemetry)
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
