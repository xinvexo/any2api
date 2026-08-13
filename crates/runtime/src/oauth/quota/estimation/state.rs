use any2api_domain::RequestTelemetryPosition;
use any2api_provider::api::{OAuthQuotaWindow, OAuthQuotaWindowKind};
use serde::{Deserialize, Deserializer, Serialize};

const MAX_WINDOWS: usize = 64;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;
const MAX_SUBSCRIPTION_TIER_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::oauth::quota) struct QuotaEstimatorState {
    pub(super) credential_fingerprint: String,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub(super) subscription_tier: Option<String>,
    pub(super) windows: Vec<QuotaWindowState>,
}

impl QuotaEstimatorState {
    pub(super) fn new(credential_fingerprint: String) -> Self {
        Self {
            credential_fingerprint,
            subscription_tier: None,
            windows: Vec::new(),
        }
    }

    pub(in crate::oauth::quota) fn valid(&self) -> bool {
        !self.credential_fingerprint.is_empty()
            && self.credential_fingerprint.len() <= 128
            && self
                .subscription_tier
                .as_ref()
                .is_none_or(|tier| !tier.is_empty() && tier.len() <= MAX_SUBSCRIPTION_TIER_BYTES)
            && self.windows.len() <= MAX_WINDOWS
            && self.windows.iter().all(QuotaWindowState::valid)
            && self.windows.iter().enumerate().all(|(index, window)| {
                self.windows[index + 1..]
                    .iter()
                    .all(|other| other.key != window.key)
            })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuotaWindowKey {
    pub(super) id: String,
    pub(super) kind: OAuthQuotaWindowKind,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub(super) limit_window_seconds: Option<u64>,
}

impl QuotaWindowKey {
    pub(super) fn from_window(window: &OAuthQuotaWindow) -> Self {
        Self {
            id: window.id.clone(),
            kind: window.kind,
            limit_window_seconds: window.limit_window_seconds,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuotaObservationAnchor {
    pub(super) used_percent: f64,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub(super) reset_at: Option<i64>,
    pub(super) position: RequestTelemetryPosition,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuotaWindowState {
    pub(super) key: QuotaWindowKey,
    pub(super) anchor: QuotaObservationAnchor,
    pub(super) total_delta_used_percent: f64,
    pub(super) total_local_cost_credits: f64,
    pub(super) completed_interval_count: u32,
}

impl QuotaWindowState {
    pub(super) fn capacity_credits(&self) -> Option<f64> {
        if self.completed_interval_count == 0 {
            return None;
        }
        let capacity = self.total_local_cost_credits * 100.0 / self.total_delta_used_percent;
        (capacity.is_finite() && capacity > 0.0).then_some(capacity)
    }

    fn valid(&self) -> bool {
        !self.key.id.trim().is_empty()
            && self.key.id.len() <= MAX_SAFE_TEXT_BYTES
            && self.anchor.used_percent.is_finite()
            && self.anchor.used_percent >= 0.0
            && self.anchor.reset_at.is_none_or(|value| value >= 0)
            && statistics_valid(self)
    }
}

fn statistics_valid(state: &QuotaWindowState) -> bool {
    if state.completed_interval_count == 0 {
        return state.total_delta_used_percent == 0.0 && state.total_local_cost_credits == 0.0;
    }
    state.total_delta_used_percent.is_finite()
        && state.total_delta_used_percent > 0.0
        && state.total_local_cost_credits.is_finite()
        && state.total_local_cost_credits > 0.0
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
