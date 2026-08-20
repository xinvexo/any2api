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
    #[serde(deserialize_with = "deserialize_nullable")]
    pub(super) estimated_included_cost_nanos: Option<u64>,
    pub(super) last_used_percent: f64,
    pub(super) credits_takeover: bool,
    pub(super) capacity_eligible: bool,
}

impl QuotaWindowState {
    pub(super) fn measured(
        key: QuotaWindowKey,
        cycle: OfficialQuotaCycle,
        estimated_included_cost_nanos: Option<u64>,
        last_used_percent: f64,
        credits_takeover: bool,
        capacity_eligible: bool,
    ) -> Self {
        Self {
            key,
            cycle_started_at_ms: cycle.started_at_ms,
            cycle_reset_at: cycle.reset_at,
            estimated_included_cost_nanos,
            last_used_percent,
            credits_takeover,
            capacity_eligible,
        }
    }

    pub(super) fn matches_cycle(&self, cycle: OfficialQuotaCycle) -> bool {
        self.cycle_started_at_ms == cycle.started_at_ms && self.cycle_reset_at == cycle.reset_at
    }

    pub(super) fn block_capacity(&mut self) {
        self.capacity_eligible = false;
    }

    pub(super) fn included_cost_credits(&self) -> Option<f64> {
        self.estimated_included_cost_nanos
            .map(|cost| cost as f64 / NANOS_PER_CREDIT)
    }

    pub(super) fn capacity_cost_nanos(&self) -> Option<u64> {
        if !self.capacity_eligible {
            return None;
        }
        let included_cost = self.estimated_included_cost_nanos?;
        if included_cost == 0 {
            return None;
        }
        if self.credits_takeover {
            return Some(included_cost);
        }
        if !self.last_used_percent.is_finite() || self.last_used_percent < MIN_CAPACITY_USED_PERCENT
        {
            return None;
        }
        let capacity = included_cost as f64 * 100.0 / self.last_used_percent;
        (capacity.is_finite() && capacity > 0.0 && capacity <= u64::MAX as f64)
            .then(|| capacity.round() as u64)
    }

    pub(super) fn capacity_credits(&self) -> Option<f64> {
        self.capacity_cost_nanos()
            .map(|cost| cost as f64 / NANOS_PER_CREDIT)
    }

    fn valid(&self) -> bool {
        !self.key.id.trim().is_empty()
            && self.key.id.len() <= MAX_SAFE_TEXT_BYTES
            && self.cycle_reset_at >= 0
            && self.last_used_percent.is_finite()
            && self.last_used_percent >= 0.0
            && (self.credits_takeover || self.estimated_included_cost_nanos.is_some())
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
