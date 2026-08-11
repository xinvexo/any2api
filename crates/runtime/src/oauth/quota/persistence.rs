use std::sync::Arc;

use any2api_domain::OAuthAccountId;
use any2api_provider::api::OAuthQuotaUsage;
use any2api_storage::api::{
    MAX_OAUTH_QUOTA_SNAPSHOT_BYTES, OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
    OAuthQuotaSnapshotRepository, StoredOAuthQuotaSnapshot,
};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use super::types::{OAuthQuotaError, OAuthQuotaSnapshot, OAuthQuotaUsdEstimate};

const MAX_WINDOWS: usize = 64;
const MAX_RESET_CREDITS: usize = 1_024;
const MAX_TEAM_REASONS: usize = 256;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;
const MAX_CREDIT_BALANCE_BYTES: usize = 128;
const MAX_ESTIMATES: usize = MAX_WINDOWS;

#[derive(Deserialize, Serialize)]
struct PersistedSnapshotPayload {
    usage: OAuthQuotaUsage,
    usd_estimates: Vec<OAuthQuotaUsdEstimate>,
}

pub(super) struct OAuthQuotaPersistence {
    repository: Arc<dyn OAuthQuotaSnapshotRepository>,
    changes: watch::Sender<u64>,
}

impl OAuthQuotaPersistence {
    pub(super) fn new(repository: Arc<dyn OAuthQuotaSnapshotRepository>) -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            repository,
            changes,
        }
    }

    pub(super) async fn load(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<OAuthQuotaSnapshot>, OAuthQuotaError> {
        let Some(stored) = self
            .repository
            .load_oauth_quota_snapshot(id)
            .await
            .map_err(|error| OAuthQuotaError::Persistence(Arc::new(error)))?
        else {
            return Ok(None);
        };
        let payload = serde_json::from_slice::<PersistedSnapshotPayload>(&stored.payload)
            .map_err(|_| OAuthQuotaError::InvalidPersistedSnapshot)?;
        validate_usage(&payload.usage)?;
        validate_estimates(&payload.usd_estimates)?;
        Ok(Some(OAuthQuotaSnapshot {
            usage: payload.usage,
            usd_estimates: payload.usd_estimates,
            fetched_at: stored.fetched_at,
        }))
    }

    pub(super) async fn store(
        &self,
        id: OAuthAccountId,
        snapshot: &OAuthQuotaSnapshot,
    ) -> Result<(), OAuthQuotaError> {
        validate_usage(&snapshot.usage)?;
        validate_estimates(&snapshot.usd_estimates)?;
        let payload = serde_json::to_vec(&PersistedSnapshotPayload {
            usage: snapshot.usage.clone(),
            usd_estimates: snapshot.usd_estimates.clone(),
        })
        .map_err(|_| OAuthQuotaError::InvalidPersistedSnapshot)?;
        if payload.len() > MAX_OAUTH_QUOTA_SNAPSHOT_BYTES {
            return Err(OAuthQuotaError::InvalidPersistedSnapshot);
        }
        self.repository
            .upsert_oauth_quota_snapshot(&StoredOAuthQuotaSnapshot {
                oauth_account_id: id,
                schema_version: OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
                fetched_at: snapshot.fetched_at,
                payload,
            })
            .await
            .map_err(|error| OAuthQuotaError::Persistence(Arc::new(error)))?;
        self.notify_changed();
        Ok(())
    }

    pub(super) async fn delete(&self, id: OAuthAccountId) -> Result<(), OAuthQuotaError> {
        if self
            .repository
            .delete_oauth_quota_snapshot(id)
            .await
            .map_err(|error| OAuthQuotaError::Persistence(Arc::new(error)))?
        {
            self.notify_changed();
        }
        Ok(())
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn notify_changed(&self) {
        self.changes
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }
}

fn validate_usage(usage: &OAuthQuotaUsage) -> Result<(), OAuthQuotaError> {
    if usage.rate_limit.as_ref().is_some_and(|rate| {
        rate.windows.len() > MAX_WINDOWS
            || rate.windows.iter().any(|window| {
                window.id.is_empty()
                    || window.id.len() > MAX_SAFE_TEXT_BYTES
                    || !window.used_percent.is_finite()
                    || window.used_percent < 0.0
            })
    }) || usage.reset_credits.as_ref().is_some_and(|credits| {
        credits.credits.len() > MAX_RESET_CREDITS
            || credits
                .credits
                .iter()
                .any(|credit| credit.expires_at.len() > MAX_SAFE_TEXT_BYTES)
    }) || usage.account_status.as_ref().is_some_and(|status| {
        status.team_blocked_reasons.len() > MAX_TEAM_REASONS
            || status
                .user_blocked_reason
                .as_ref()
                .is_some_and(|value| value.len() > MAX_SAFE_TEXT_BYTES)
            || status
                .team_blocked_reasons
                .iter()
                .any(|value| value.len() > MAX_SAFE_TEXT_BYTES)
    }) || usage.credits.as_ref().is_some_and(|credits| {
        credits.balance.as_ref().is_some_and(|value| {
            value.len() > MAX_CREDIT_BALANCE_BYTES || !is_non_negative_decimal(value)
        })
    }) {
        return Err(OAuthQuotaError::InvalidPersistedSnapshot);
    }
    Ok(())
}

fn validate_estimates(estimates: &[OAuthQuotaUsdEstimate]) -> Result<(), OAuthQuotaError> {
    if estimates.len() > MAX_ESTIMATES
        || estimates.iter().any(|estimate| {
            estimate.window_id.trim().is_empty()
                || estimate.window_id.len() > MAX_SAFE_TEXT_BYTES
                || estimate.pricing_basis.trim().is_empty()
                || estimate.pricing_basis.len() > MAX_SAFE_TEXT_BYTES
                || estimate.window_reset_at.is_some_and(|value| value < 0)
                || estimate.sample_started_at < 0
                || estimate.sample_ended_at < estimate.sample_started_at
                || !positive_finite(estimate.estimated_capacity_usd)
                || !non_negative_finite(estimate.estimated_used_usd)
                || !non_negative_finite(estimate.estimated_remaining_usd)
                || !positive_finite(estimate.sample_cost_usd)
                || !positive_finite(estimate.sample_used_percent_delta)
        })
    {
        return Err(OAuthQuotaError::InvalidPersistedSnapshot);
    }
    Ok(())
}

fn positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn non_negative_finite(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn is_non_negative_decimal(value: &str) -> bool {
    let mut decimal_seen = false;
    let mut digit_seen = false;
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => digit_seen = true,
            b'.' if !decimal_seen => decimal_seen = true,
            _ => return false,
        }
    }
    digit_seen && !value.ends_with('.')
}
