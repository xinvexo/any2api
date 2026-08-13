use any2api_provider::api::{OAuthQuotaWindow, OAuthQuotaWindowKind};
use serde::{Deserialize, Deserializer, Serialize};

use crate::request_telemetry::RequestTelemetryObservation;

use super::super::types::OAuthQuotaIntervalDiagnostic;

const MAX_WINDOWS: usize = 64;
pub(super) const MAX_ACCEPTED_SAMPLES: usize = 9;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;
const MAX_RATE_CARDS_PER_SAMPLE: usize = 16;
const MAX_RATE_CARD_BYTES: usize = 128;
const MAX_SUBSCRIPTION_TIER_BYTES: usize = 128;

/// Estimator state for one OAuth account. Every request the account serves
/// goes through this process, so local telemetry is the complete consumption
/// record and each clean interval yields an unbiased capacity measurement
/// (ADR-0144).
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::oauth::quota) struct QuotaEstimatorState {
    pub(super) credential_fingerprint: String,
    /// Part of the capacity signature: a plan change means the learned
    /// capacity no longer describes this account.
    #[serde(deserialize_with = "deserialize_nullable")]
    pub(super) subscription_tier: Option<String>,
    pub(super) next_epoch: u64,
    pub(super) windows: Vec<QuotaWindowState>,
}

impl QuotaEstimatorState {
    pub(super) fn new(credential_fingerprint: String) -> Self {
        Self {
            credential_fingerprint,
            subscription_tier: None,
            next_epoch: 1,
            windows: Vec::new(),
        }
    }

    pub(super) fn allocate_epoch(&mut self) -> u64 {
        let epoch = self.next_epoch.max(1);
        self.next_epoch = epoch.saturating_add(1);
        epoch
    }

    pub(in crate::oauth::quota) fn valid(&self) -> bool {
        !self.credential_fingerprint.is_empty()
            && self.credential_fingerprint.len() <= 128
            && self
                .subscription_tier
                .as_ref()
                .is_none_or(|tier| !tier.is_empty() && tier.len() <= MAX_SUBSCRIPTION_TIER_BYTES)
            && self.next_epoch >= 1
            && self.windows.len() <= MAX_WINDOWS
            && self.windows.iter().all(QuotaWindowState::valid)
            && self
                .windows
                .iter()
                .all(|window| window.epoch < self.next_epoch)
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
    pub(super) telemetry: RequestTelemetryObservation,
}

/// One capacity measurement: `capacity = local_cost × 100 / Δused%` over a
/// fence-complete interval. The percent delta doubles as the aggregation
/// weight because official percent quantization perturbs small denominators
/// more than large ones.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuotaCapacitySample {
    pub(super) capacity_credits: f64,
    pub(super) delta_used_percent: f64,
    pub(super) local_cost_credits: f64,
    pub(super) observed_at_ms: u64,
    pub(super) epoch: u64,
    pub(super) rate_cards: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct QuotaWindowState {
    pub(super) key: QuotaWindowKey,
    pub(super) epoch: u64,
    pub(super) epoch_started_at_ms: u64,
    /// Advances on every observation; drives reset/rollover detection.
    pub(super) last_observation: QuotaObservationAnchor,
    /// Start of the open sample interval; held across small deltas until the
    /// accumulated Δused% is large enough to mint a sample.
    pub(super) sample_anchor: QuotaObservationAnchor,
    pub(super) samples: Vec<QuotaCapacitySample>,
    pub(super) latest_interval: OAuthQuotaIntervalDiagnostic,
}

impl QuotaWindowState {
    pub(super) fn fresh_sample_count(&self) -> usize {
        self.samples
            .iter()
            .filter(|sample| sample.epoch == self.epoch)
            .count()
    }

    fn valid(&self) -> bool {
        !self.key.id.trim().is_empty()
            && self.key.id.len() <= MAX_SAFE_TEXT_BYTES
            && self.epoch >= 1
            && anchor_valid(&self.last_observation)
            && anchor_valid(&self.sample_anchor)
            && self.last_observation.telemetry.position.process_id
                == self.sample_anchor.telemetry.position.process_id
            && self.sample_anchor.telemetry.position.sequence
                <= self.last_observation.telemetry.position.sequence
            && self.samples.len() <= MAX_ACCEPTED_SAMPLES
            && self
                .samples
                .iter()
                .all(|sample| sample_valid(sample, self.epoch))
            && diagnostic_valid(&self.latest_interval)
    }
}

fn anchor_valid(anchor: &QuotaObservationAnchor) -> bool {
    anchor.used_percent.is_finite()
        && anchor.used_percent >= 0.0
        && anchor.reset_at.is_none_or(|value| value >= 0)
        && anchor.telemetry.position.process_id == anchor.telemetry.checkpoint.process_id
}

fn sample_valid(sample: &QuotaCapacitySample, current_epoch: u64) -> bool {
    sample.capacity_credits.is_finite()
        && sample.capacity_credits > 0.0
        && sample.delta_used_percent.is_finite()
        && sample.delta_used_percent > 0.0
        && sample.local_cost_credits.is_finite()
        && sample.local_cost_credits > 0.0
        && sample.epoch >= 1
        && sample.epoch <= current_epoch
        && rate_cards_valid(&sample.rate_cards)
}

fn rate_cards_valid(rate_cards: &[String]) -> bool {
    rate_cards.len() <= MAX_RATE_CARDS_PER_SAMPLE
        && rate_cards
            .iter()
            .all(|rate_card| !rate_card.trim().is_empty() && rate_card.len() <= MAX_RATE_CARD_BYTES)
}

fn diagnostic_valid(value: &OAuthQuotaIntervalDiagnostic) -> bool {
    value.ended_at >= 0
        && value.started_at.is_none_or(|started| started >= 0)
        && value
            .delta_used_percent
            .is_none_or(|delta| delta.is_finite())
        && value
            .local_cost_credits
            .is_none_or(|cost| cost.is_finite() && cost >= 0.0)
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
