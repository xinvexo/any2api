use any2api_domain::GatewayApiKeyId;
use async_trait::async_trait;
use sqlx::{FromRow, SqliteConnection};

use crate::{error::StorageError, sqlite::SqliteStore};

pub const GATEWAY_API_KEY_RECENT_OUTCOME_LIMIT: u32 = 24;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyRequestOutcome {
    pub status_code: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyUsageSummary {
    pub id: GatewayApiKeyId,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub recent_outcomes: Vec<GatewayApiKeyRequestOutcome>,
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
        let recent_rows = sqlx::query_as::<_, GatewayApiKeyRecentOutcomeRow>(
            "SELECT gateway_api_key_id, status_code FROM ( \
             SELECT gateway_api_key_id, status_code, \
             ROW_NUMBER() OVER (PARTITION BY gateway_api_key_id \
                 ORDER BY started_at_ms DESC, request_id DESC) AS row_number \
             FROM request_logs WHERE gateway_api_key_id IS NOT NULL \
             ) WHERE row_number <= ? \
             ORDER BY gateway_api_key_id ASC, row_number DESC",
        )
        .bind(i64::from(GATEWAY_API_KEY_RECENT_OUTCOME_LIMIT))
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let mut summaries = summary_rows
            .into_iter()
            .map(parse_usage_row)
            .collect::<Result<Vec<_>, _>>()?;
        for row in recent_rows {
            let id = parse_id(&row.gateway_api_key_id)?;
            let status_code = parse_status_code(row.status_code)?;
            if let Some(summary) = summaries.iter_mut().find(|summary| summary.id == id) {
                summary
                    .recent_outcomes
                    .push(GatewayApiKeyRequestOutcome { status_code });
            }
        }
        Ok(summaries)
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
struct GatewayApiKeyRecentOutcomeRow {
    gateway_api_key_id: String,
    status_code: i64,
}

fn parse_usage_row(row: GatewayApiKeyUsageRow) -> Result<GatewayApiKeyUsageSummary, StorageError> {
    let total_requests = from_i64(row.total_requests)?;
    let successful_requests = from_i64(row.successful_requests)?;
    if successful_requests > total_requests {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(GatewayApiKeyUsageSummary {
        id: parse_id(&row.gateway_api_key_id)?,
        total_requests,
        successful_requests,
        recent_outcomes: Vec::new(),
    })
}

fn parse_id<T: std::str::FromStr>(value: &str) -> Result<T, StorageError> {
    value.parse().map_err(|_| StorageError::CorruptTelemetry)
}

fn parse_status_code(value: i64) -> Result<u16, StorageError> {
    let value = u16::try_from(value).map_err(|_| StorageError::CorruptTelemetry)?;
    (100..=599)
        .contains(&value)
        .then_some(value)
        .ok_or(StorageError::CorruptTelemetry)
}

fn from_i64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
