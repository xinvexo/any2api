use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use any2api_domain::{CredentialId, OAuthAccountId, RoutingCredentialId};
use async_trait::async_trait;
use sqlx::FromRow;

use crate::{error::StorageError, sqlite::SqliteStore};

/// Each usage bar covers this many minutes.
pub const UPSTREAM_USAGE_WINDOW_MINUTES: u64 = 2;
/// Fixed bars for the last hour (30 × 2 min), newest-last; empty = gray.
pub const UPSTREAM_USAGE_WINDOW_COUNT: usize = 30;

const WINDOW_MS: u64 = UPSTREAM_USAGE_WINDOW_MINUTES * 60 * 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialWindowSlot {
    pub started_at_ms: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamCredentialUsageSummary {
    pub id: RoutingCredentialId,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub window_slots: Vec<UpstreamCredentialWindowSlot>,
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
        let now_ms = unix_now_ms()?;
        let range_start_ms = window_range_start(now_ms);
        let mut transaction = self.pool().begin().await?;
        let summary_rows = sqlx::query_as::<_, UpstreamUsageRow>(&format!(
            "{} SELECT source, upstream_id, COUNT(*) AS total_requests, \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
             AS successful_requests FROM upstream_requests GROUP BY source, upstream_id",
            upstream_requests_cte()
        ))
        .fetch_all(&mut *transaction)
        .await?;
        let slot_rows = sqlx::query_as::<_, UpstreamWindowSlotRow>(&format!(
            "{} SELECT source, upstream_id, \
             (started_at_ms / ?) * ? AS bucket_start_ms, \
             COUNT(*) AS total_requests, \
             SUM(CASE WHEN status_code >= 200 AND status_code < 300 THEN 1 ELSE 0 END) \
             AS successful_requests \
             FROM upstream_requests \
             WHERE started_at_ms >= ? \
             GROUP BY source, upstream_id, bucket_start_ms",
            upstream_requests_cte()
        ))
        .bind(i64::try_from(WINDOW_MS).map_err(|_| StorageError::CorruptTelemetry)?)
        .bind(i64::try_from(WINDOW_MS).map_err(|_| StorageError::CorruptTelemetry)?)
        .bind(i64::try_from(range_start_ms).map_err(|_| StorageError::CorruptTelemetry)?)
        .fetch_all(&mut *transaction)
        .await?;
        transaction.commit().await?;

        let mut slots_by_id: HashMap<RoutingCredentialId, HashMap<u64, (u64, u64)>> =
            HashMap::new();
        for row in slot_rows {
            let id = parse_routing_id(&row.source, &row.upstream_id)?;
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
            .map(|row| {
                let id = parse_routing_id(&row.source, &row.upstream_id)?;
                let total_requests = from_i64(row.total_requests)?;
                let successful_requests = from_i64(row.successful_requests)?;
                if successful_requests > total_requests {
                    return Err(StorageError::CorruptTelemetry);
                }
                let filled = slots_by_id.remove(&id).unwrap_or_default();
                Ok(UpstreamCredentialUsageSummary {
                    id,
                    total_requests,
                    successful_requests,
                    window_slots: build_window_slots(now_ms, &filled),
                })
            })
            .collect()
    }
}

/// Build a fixed-length newest-last window for admin responses with no usage row.
#[must_use]
pub fn empty_upstream_window_slots(now_ms: u64) -> Vec<UpstreamCredentialWindowSlot> {
    build_window_slots(now_ms, &HashMap::new())
}

fn build_window_slots(
    now_ms: u64,
    filled: &HashMap<u64, (u64, u64)>,
) -> Vec<UpstreamCredentialWindowSlot> {
    let newest = align_window_start(now_ms);
    let oldest = newest.saturating_sub((UPSTREAM_USAGE_WINDOW_COUNT as u64 - 1) * WINDOW_MS);
    (0..UPSTREAM_USAGE_WINDOW_COUNT)
        .map(|index| {
            let started_at_ms = oldest + (index as u64) * WINDOW_MS;
            let (total_requests, successful_requests) =
                filled.get(&started_at_ms).copied().unwrap_or((0, 0));
            UpstreamCredentialWindowSlot {
                started_at_ms,
                total_requests,
                successful_requests,
            }
        })
        .collect()
}

fn window_range_start(now_ms: u64) -> u64 {
    align_window_start(now_ms).saturating_sub((UPSTREAM_USAGE_WINDOW_COUNT as u64 - 1) * WINDOW_MS)
}

fn align_window_start(now_ms: u64) -> u64 {
    (now_ms / WINDOW_MS) * WINDOW_MS
}

fn unix_now_ms() -> Result<u64, StorageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| StorageError::CorruptTelemetry)
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
struct UpstreamWindowSlotRow {
    source: String,
    upstream_id: String,
    bucket_start_ms: i64,
    total_requests: i64,
    successful_requests: i64,
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

fn from_i64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::CorruptTelemetry)
}
