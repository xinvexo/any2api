use any2api_domain::OAuthAccountId;
use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthQuotaRequestLogModelUsage {
    pub public_model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OAuthQuotaRequestLogUsage {
    pub records_complete: bool,
    pub models: Vec<OAuthQuotaRequestLogModelUsage>,
}

#[async_trait]
pub trait OAuthQuotaEstimationRepository: Send + Sync {
    async fn oauth_quota_request_log_usage(
        &self,
        id: OAuthAccountId,
        started_at_ms: u64,
        ended_at_ms: u64,
    ) -> Result<OAuthQuotaRequestLogUsage, StorageError>;

    async fn load_oauth_quota_reset_boundary(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<u64>, StorageError>;

    async fn record_oauth_quota_reset(
        &self,
        id: OAuthAccountId,
        reset_at_ms: u64,
    ) -> Result<(), StorageError>;
}

#[derive(sqlx::FromRow)]
struct UsageRow {
    public_model: Option<String>,
    incomplete_records: i64,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
}

#[async_trait]
impl OAuthQuotaEstimationRepository for SqliteStore {
    async fn oauth_quota_request_log_usage(
        &self,
        id: OAuthAccountId,
        started_at_ms: u64,
        ended_at_ms: u64,
    ) -> Result<OAuthQuotaRequestLogUsage, StorageError> {
        if started_at_ms >= ended_at_ms {
            return Ok(OAuthQuotaRequestLogUsage::default());
        }
        let rows = sqlx::query_as::<_, UsageRow>(
            "SELECT public_model, \
             SUM(CASE WHEN public_model IS NULL OR input_tokens IS NULL OR output_tokens IS NULL \
                 THEN 1 ELSE 0 END) AS incomplete_records, \
             COALESCE(SUM(input_tokens), 0) AS input_tokens, \
             COALESCE(SUM(output_tokens), 0) AS output_tokens, \
             COALESCE(SUM(cache_read_tokens), 0) AS cache_read_tokens \
             FROM request_logs \
             WHERE oauth_account_id = ? AND started_at_ms >= ? AND started_at_ms < ? \
             GROUP BY public_model ORDER BY public_model ASC",
        )
        .bind(id.to_string())
        .bind(to_i64(started_at_ms)?)
        .bind(to_i64(ended_at_ms)?)
        .fetch_all(self.pool())
        .await?;

        let records_complete = rows.iter().all(|row| row.incomplete_records == 0);
        let models = rows
            .into_iter()
            .filter_map(|row| {
                row.public_model.map(|public_model| {
                    Ok(OAuthQuotaRequestLogModelUsage {
                        public_model,
                        input_tokens: to_u64(row.input_tokens)?,
                        output_tokens: to_u64(row.output_tokens)?,
                        cache_read_tokens: to_u64(row.cache_read_tokens)?,
                    })
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        Ok(OAuthQuotaRequestLogUsage {
            records_complete,
            models,
        })
    }

    async fn load_oauth_quota_reset_boundary(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<u64>, StorageError> {
        sqlx::query_scalar::<_, i64>(
            "SELECT reset_at_ms FROM oauth_quota_estimation_boundaries \
             WHERE oauth_account_id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(self.pool())
        .await?
        .map(to_u64)
        .transpose()
    }

    async fn record_oauth_quota_reset(
        &self,
        id: OAuthAccountId,
        reset_at_ms: u64,
    ) -> Result<(), StorageError> {
        let mut transaction = self.begin_write().await?;
        sqlx::query(
            "INSERT INTO oauth_quota_estimation_boundaries (oauth_account_id, reset_at_ms) \
             VALUES (?, ?) ON CONFLICT(oauth_account_id) DO UPDATE SET \
             reset_at_ms = MAX(reset_at_ms, excluded.reset_at_ms), \
             updated_at = CURRENT_TIMESTAMP",
        )
        .bind(id.to_string())
        .bind(to_i64(reset_at_ms)?)
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM oauth_quota_snapshots WHERE oauth_account_id = ?")
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}

fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
