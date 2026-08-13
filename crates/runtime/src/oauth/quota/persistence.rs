use std::sync::Arc;

use any2api_domain::OAuthAccountId;
use any2api_provider::api::OAuthQuotaUsage;
use any2api_storage::api::{
    MAX_OAUTH_QUOTA_SNAPSHOT_BYTES, OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
    OAuthQuotaSnapshotRepository, StoredOAuthQuotaSnapshot,
};
use serde::{Deserialize, Deserializer, Serialize};
use tokio::sync::watch;

use super::{estimation::state::QuotaEstimatorState, types::OAuthQuotaError};

const MAX_WINDOWS: usize = 64;
const MAX_RESET_CREDITS: usize = 1_024;
const MAX_TEAM_REASONS: usize = 256;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;
const MAX_CREDIT_BALANCE_BYTES: usize = 128;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedSnapshotPayload {
    usage: OAuthQuotaUsage,
    #[serde(deserialize_with = "deserialize_nullable")]
    estimator_state: Option<QuotaEstimatorState>,
}

pub(super) struct StoredQuotaTelemetry {
    pub(super) usage: OAuthQuotaUsage,
    pub(super) estimator_state: Option<QuotaEstimatorState>,
    pub(super) fetched_at: i64,
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
    ) -> Result<Option<StoredQuotaTelemetry>, OAuthQuotaError> {
        let Some(stored) = self
            .repository
            .load_oauth_quota_snapshot(id)
            .await
            .map_err(|error| OAuthQuotaError::Persistence(Arc::new(error)))?
        else {
            return Ok(None);
        };
        let payload = decode_payload(&stored.payload)?;
        validate_usage(&payload.usage)?;
        if payload
            .estimator_state
            .as_ref()
            .is_some_and(|state| !state.valid())
        {
            return Err(OAuthQuotaError::InvalidPersistedSnapshot);
        }
        Ok(Some(StoredQuotaTelemetry {
            usage: payload.usage,
            estimator_state: payload.estimator_state,
            fetched_at: stored.fetched_at,
        }))
    }

    pub(super) async fn store(
        &self,
        id: OAuthAccountId,
        telemetry: &StoredQuotaTelemetry,
    ) -> Result<(), OAuthQuotaError> {
        validate_usage(&telemetry.usage)?;
        if telemetry
            .estimator_state
            .as_ref()
            .is_some_and(|state| !state.valid())
            || telemetry.fetched_at < 0
        {
            return Err(OAuthQuotaError::InvalidPersistedSnapshot);
        }
        let payload = serde_json::to_vec(&PersistedSnapshotPayload {
            usage: telemetry.usage.clone(),
            estimator_state: telemetry.estimator_state.clone(),
        })
        .map_err(|_| OAuthQuotaError::InvalidPersistedSnapshot)?;
        if payload.len() > MAX_OAUTH_QUOTA_SNAPSHOT_BYTES {
            return Err(OAuthQuotaError::InvalidPersistedSnapshot);
        }
        self.repository
            .upsert_oauth_quota_snapshot(&StoredOAuthQuotaSnapshot {
                oauth_account_id: id,
                schema_version: OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
                fetched_at: telemetry.fetched_at,
                payload,
            })
            .await
            .map_err(|error| OAuthQuotaError::Persistence(Arc::new(error)))?;
        self.notify_changed();
        Ok(())
    }

    pub(super) async fn delete(&self, id: OAuthAccountId) -> Result<(), OAuthQuotaError> {
        self.repository
            .delete_oauth_quota_snapshot(id)
            .await
            .map_err(|error| OAuthQuotaError::Persistence(Arc::new(error)))?;
        self.notify_changed();
        Ok(())
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    pub(super) fn notify_changed(&self) {
        self.changes
            .send_modify(|epoch| *epoch = epoch.wrapping_add(1));
    }
}

fn decode_payload(payload: &[u8]) -> Result<PersistedSnapshotPayload, OAuthQuotaError> {
    serde_json::from_slice(payload).map_err(|_| OAuthQuotaError::InvalidPersistedSnapshot)
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
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
    }) || unsafe_account_status(usage)
        || usage.credits.as_ref().is_some_and(|credits| {
            credits.balance.as_ref().is_some_and(|value| {
                value.len() > MAX_CREDIT_BALANCE_BYTES || !is_non_negative_decimal(value)
            })
        })
    {
        return Err(OAuthQuotaError::InvalidPersistedSnapshot);
    }
    Ok(())
}

fn unsafe_account_status(usage: &OAuthQuotaUsage) -> bool {
    usage.account_status.as_ref().is_some_and(|status| {
        status.team_blocked_reasons.len() > MAX_TEAM_REASONS
            || status
                .user_blocked_reason
                .as_ref()
                .is_some_and(|value| value.len() > MAX_SAFE_TEXT_BYTES)
            || status
                .team_blocked_reasons
                .iter()
                .any(|value| value.len() > MAX_SAFE_TEXT_BYTES)
    })
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

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::decode_payload;

    fn current_payload() -> Value {
        json!({
            "usage": {
                "rate_limit": null,
                "credits": null,
                "access": null,
                "reset_credits": null,
                "billing": null,
                "token_balance": null,
                "subscription_tier": null,
                "account_status": null
            },
            "estimator_state": {
                "credential_fingerprint": "fingerprint",
                "subscription_tier": null,
                "next_epoch": 1,
                "windows": []
            }
        })
    }

    #[test]
    fn current_snapshot_payload_decodes() {
        let payload = serde_json::to_vec(&current_payload()).expect("serialize fixture");
        decode_payload(&payload).expect("decode current payload");
    }

    #[test]
    fn historical_root_fields_are_rejected() {
        let mut payload = current_payload();
        payload["usd_estimates"] = json!([]);
        let payload = serde_json::to_vec(&payload).expect("serialize fixture");
        assert!(decode_payload(&payload).is_err());
    }

    #[test]
    fn unknown_nested_usage_fields_are_rejected() {
        let mut payload = current_payload();
        payload["usage"]["legacy_balance"] = json!(0);
        let payload = serde_json::to_vec(&payload).expect("serialize fixture");
        assert!(decode_payload(&payload).is_err());
    }

    #[test]
    fn omitted_current_nullable_fields_are_rejected() {
        let mut payload = current_payload();
        payload
            .as_object_mut()
            .expect("root object")
            .remove("estimator_state");
        let payload = serde_json::to_vec(&payload).expect("serialize fixture");
        assert!(decode_payload(&payload).is_err());

        let mut payload = current_payload();
        payload["usage"]
            .as_object_mut()
            .expect("usage object")
            .remove("billing");
        let payload = serde_json::to_vec(&payload).expect("serialize fixture");
        assert!(decode_payload(&payload).is_err());
    }

    #[test]
    fn historical_estimator_fields_are_rejected() {
        let mut payload = current_payload();
        payload["estimator_state"]["pending_high_candidate"] = json!(null);
        let payload = serde_json::to_vec(&payload).expect("serialize fixture");
        assert!(decode_payload(&payload).is_err());
    }
}
