use any2api_provider::api::{OAuthQuotaWindow, OAuthQuotaWindowKind};
use serde::{Deserialize, Deserializer, Serialize};

pub(super) const MAX_WINDOWS: usize = 64;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;
const MAX_SUBSCRIPTION_TIER_BYTES: usize = 128;
const MILLIS_PER_SECOND: u64 = 1_000;
const NANOS_PER_CREDIT: f64 = 1_000_000_000.0;
pub(super) const MIN_CAPACITY_USED_PERCENT: f64 = 2.0;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OfficialQuotaCycle {
    pub(super) started_at_ms: u64,
    pub(super) reset_at: i64,
}

impl OfficialQuotaCycle {
    pub(super) fn from_window(window: &OAuthQuotaWindow) -> Option<Self> {
        let reset_at = window.reset_at?;
        let started_at_ms = cycle_started_at_ms(window.limit_window_seconds?, reset_at)?;
        Some(Self {
            started_at_ms,
            reset_at,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuotaWindowState {
    pub(super) key: QuotaWindowKey,
    pub(super) cycle_started_at_ms: u64,
    pub(super) cycle_reset_at: i64,
    pub(super) local_cost_nanos: u64,
    pub(super) capacity_eligible: bool,
}

impl QuotaWindowState {
    pub(super) fn measured(
        key: QuotaWindowKey,
        cycle: OfficialQuotaCycle,
        local_cost_nanos: u64,
        capacity_eligible: bool,
    ) -> Self {
        Self {
            key,
            cycle_started_at_ms: cycle.started_at_ms,
            cycle_reset_at: cycle.reset_at,
            local_cost_nanos,
            capacity_eligible,
        }
    }

    pub(super) fn matches_cycle(&self, cycle: OfficialQuotaCycle) -> bool {
        self.cycle_started_at_ms == cycle.started_at_ms && self.cycle_reset_at == cycle.reset_at
    }

    pub(super) fn block_capacity(&mut self) {
        self.capacity_eligible = false;
    }

    pub(super) fn local_cost_credits(&self) -> f64 {
        self.local_cost_nanos as f64 / NANOS_PER_CREDIT
    }

    pub(super) fn capacity_credits(&self, used_percent: f64) -> Option<f64> {
        if !self.capacity_eligible
            || !used_percent.is_finite()
            || used_percent < MIN_CAPACITY_USED_PERCENT
        {
            return None;
        }
        let local_cost = self.local_cost_credits();
        if local_cost <= 0.0 {
            return None;
        }
        let capacity = local_cost * 100.0 / used_percent;
        (capacity.is_finite() && capacity > 0.0).then_some(capacity)
    }

    fn valid(&self) -> bool {
        !self.key.id.trim().is_empty()
            && self.key.id.len() <= MAX_SAFE_TEXT_BYTES
            && self.cycle_reset_at >= 0
            && self
                .key
                .limit_window_seconds
                .and_then(|duration| cycle_started_at_ms(duration, self.cycle_reset_at))
                == Some(self.cycle_started_at_ms)
    }
}

fn cycle_started_at_ms(limit_window_seconds: u64, reset_at: i64) -> Option<u64> {
    if limit_window_seconds == 0 {
        return None;
    }
    let reset_at = u64::try_from(reset_at).ok()?;
    let started_at = reset_at.checked_sub(limit_window_seconds)?;
    started_at.checked_mul(MILLIS_PER_SECOND)
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
