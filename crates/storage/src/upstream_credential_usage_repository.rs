use any2api_domain::{CredentialId, OAuthAccountId, RoutingCredentialId};
use async_trait::async_trait;
use sqlx::FromRow;

use crate::{error::StorageError, sqlite::SqliteStore};

pub const UPSTREAM_CREDENTIAL_RECENT_OUTCOME_LIMIT: u32 = 24;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialRequestOutcome {
    pub status_code: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialUsageSummary {
    pub id: RoutingCredentialId,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub recent_outcomes: Vec<UpstreamCredentialRequestOutcome>,
}

impl UpstreamCredentialUsageSummary {
    #[must_use]
    pub fn failed_requests(&self) -> u64 {
        self.total_requests.saturating_sub(self.successful_requests)
    }
}

#[async_trait]
pub trait UpstreamCredentialUsageRepository: Send + Sync {
    async fn list_upstream_credential_usage(
        &self,
    ) -> Result<Vec<UpstreamCredentialUsageSummary>, StorageError>;
}

#[async_trait]
impl UpstreamCredentialUsageRepository for SqliteStore {
    async fn list_upstream_credential_usage(
        &self,
    ) -> Result<Vec<UpstreamCredentialUsageSummary>, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let summary_rows = sqlx::query_as::<_, UpstreamUsageRow>(&format!(
            "{} SELECT source, upstream_id, COUNT(*) AS total_requests, \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
             AS successful_requests FROM upstream_requests GROUP BY source, upstream_id",
            upstream_requests_cte()
        ))
        .fetch_all(&mut *transaction)
        .await?;
        let recent_rows = sqlx::query_as::<_, UpstreamRecentOutcomeRow>(&format!(
            "{}, ranked AS (SELECT source, upstream_id, status_code, \
             ROW_NUMBER() OVER (PARTITION BY source, upstream_id \
                 ORDER BY started_at_ms DESC, request_id DESC) AS row_number \
             FROM upstream_requests) \
             SELECT source, upstream_id, status_code FROM ranked WHERE row_number <= ? \
             ORDER BY source ASC, upstream_id ASC, row_number DESC",
            upstream_requests_cte()
        ))
        .bind(i64::from(UPSTREAM_CREDENTIAL_RECENT_OUTCOME_LIMIT))
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let mut summaries = summary_rows
            .into_iter()
            .map(parse_usage_row)
            .collect::<Result<Vec<_>, _>>()?;
        for row in recent_rows {
            let id = parse_routing_id(&row.source, &row.upstream_id)?;
            let status_code = parse_status_code(row.status_code)?;
            if let Some(summary) = summaries.iter_mut().find(|summary| summary.id == id) {
                summary
                    .recent_outcomes
                    .push(UpstreamCredentialRequestOutcome { status_code });
            }
        }
        Ok(summaries)
    }
}

fn upstream_requests_cte() -> &'static str {
    "WITH upstream_requests AS ( \
     SELECT 'provider_credential' AS source, credential_id AS upstream_id, status_code, \
            started_at_ms, request_id FROM request_logs WHERE credential_id IS NOT NULL \
     UNION ALL \
     SELECT 'oauth_account' AS source, oauth_account_id AS upstream_id, status_code, \
            started_at_ms, request_id FROM request_logs WHERE oauth_account_id IS NOT NULL)"
}

#[derive(FromRow)]
struct UpstreamUsageRow {
    source: String,
    upstream_id: String,
    total_requests: i64,
    successful_requests: i64,
}

#[derive(FromRow)]
struct UpstreamRecentOutcomeRow {
    source: String,
    upstream_id: String,
    status_code: i64,
}

fn parse_usage_row(row: UpstreamUsageRow) -> Result<UpstreamCredentialUsageSummary, StorageError> {
    let total_requests = from_i64(row.total_requests)?;
    let successful_requests = from_i64(row.successful_requests)?;
    if successful_requests > total_requests {
        return Err(StorageError::CorruptTelemetry);
    }
    Ok(UpstreamCredentialUsageSummary {
        id: parse_routing_id(&row.source, &row.upstream_id)?,
        total_requests,
        successful_requests,
        recent_outcomes: Vec::new(),
    })
}

fn parse_routing_id(source: &str, value: &str) -> Result<RoutingCredentialId, StorageError> {
    match source {
        "provider_credential" => value
            .parse::<CredentialId>()
            .map(RoutingCredentialId::provider_credential),
        "oauth_account" => value
            .parse::<OAuthAccountId>()
            .map(RoutingCredentialId::oauth_account),
        _ => return Err(StorageError::CorruptTelemetry),
    }
    .map_err(|_| StorageError::CorruptTelemetry)
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
