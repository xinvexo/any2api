use std::sync::Arc;

use any2api_domain::OAuthAccountId;
use any2api_provider::api::OAuthQuotaUsage;
use any2api_storage::api::{
    MAX_OAUTH_QUOTA_SNAPSHOT_BYTES, OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
    OAuthQuotaSnapshotRepository, StoredOAuthQuotaSnapshot,
};
use tokio::sync::watch;

use super::types::{OAuthQuotaError, OAuthQuotaSnapshot};

const MAX_WINDOWS: usize = 64;
const MAX_RESET_CREDITS: usize = 1_024;
const MAX_TEAM_REASONS: usize = 256;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;

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
            .map_err(OAuthQuotaError::Persistence)?
        else {
            return Ok(None);
        };
        let usage = serde_json::from_slice::<OAuthQuotaUsage>(&stored.payload)
            .map_err(|_| OAuthQuotaError::InvalidPersistedSnapshot)?;
        validate_usage(&usage)?;
        Ok(Some(OAuthQuotaSnapshot {
            usage,
            fetched_at: stored.fetched_at,
        }))
    }

    pub(super) async fn store(
        &self,
        id: OAuthAccountId,
        snapshot: &OAuthQuotaSnapshot,
    ) -> Result<(), OAuthQuotaError> {
        validate_usage(&snapshot.usage)?;
        let payload = serde_json::to_vec(&snapshot.usage)
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
            .map_err(OAuthQuotaError::Persistence)?;
        self.notify_changed();
        Ok(())
    }

    pub(super) async fn delete(&self, id: OAuthAccountId) -> Result<(), OAuthQuotaError> {
        if self
            .repository
            .delete_oauth_quota_snapshot(id)
            .await
            .map_err(OAuthQuotaError::Persistence)?
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
    }) {
        return Err(OAuthQuotaError::InvalidPersistedSnapshot);
    }
    Ok(())
}
