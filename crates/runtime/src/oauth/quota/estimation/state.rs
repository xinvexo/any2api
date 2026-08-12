use any2api_provider::api::{OAuthQuotaWindow, OAuthQuotaWindowKind};
use serde::{Deserialize, Serialize};

use crate::request_telemetry::RequestTelemetryObservation;

use super::super::types::OAuthQuotaIntervalDiagnostic;

const MAX_WINDOWS: usize = 64;
pub(super) const MAX_ACCEPTED_SAMPLES: usize = 9;
const MAX_SAFE_TEXT_BYTES: usize = 4_096;
const MAX_RATE_CARDS_PER_SAMPLE: usize = 16;
const MAX_RATE_CARD_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(in crate::oauth::quota) struct QuotaEstimatorState {
    pub(super) credential_fingerprint: String,
    pub(super) next_epoch: u64,
    pub(super) windows: Vec<QuotaWindowState>,
}

impl QuotaEstimatorState {
    pub(super) fn new(credential_fingerprint: String) -> Self {
        Self {
            credential_fingerprint,
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
pub(super) struct QuotaWindowKey {
    pub(super) id: String,
    pub(super) kind: OAuthQuotaWindowKind,
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
pub(super) struct QuotaObservationAnchor {
    pub(super) used_percent: f64,
    pub(super) reset_at: Option<i64>,
    pub(super) telemetry: RequestTelemetryObservation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct QuotaCapacitySample {
    pub(super) capacity_credits: f64,
    pub(super) observed_at_ms: u64,
    pub(super) epoch: u64,
    pub(super) rate_cards: Vec<String>,
}

/// Fence-verified clean prefix of the current sample interval: everything from
/// the sample anchor up to the most recent clean probe. Salvaged as a capacity
/// sample when the segment is cut short by a gap, contamination or reset.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct QuotaSegmentProgress {
    pub(super) ended_used_percent: f64,
    pub(super) ended_at_ms: u64,
    pub(super) cost_nanos: u64,
    pub(super) priced_request_count: u64,
    pub(super) rate_cards: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct QuotaWindowState {
    pub(super) key: QuotaWindowKey,
    pub(super) epoch: u64,
    pub(super) epoch_started_at_ms: u64,
    pub(super) last_observation: QuotaObservationAnchor,
    pub(super) sample_anchor: QuotaObservationAnchor,
    pub(super) segment: Option<QuotaSegmentProgress>,
    pub(super) samples: Vec<QuotaCapacitySample>,
    /// Above-band candidate awaiting a second consistent high sample before
    /// the cluster is revised upward; lower-bound semantics never revise the
    /// cluster downward without a capacity-signature event.
    pub(super) pending_high: Option<QuotaCapacitySample>,
    /// Consecutive below-band or costless-delta candidates; degrades
    /// confidence without touching the capacity cluster.
    pub(super) low_streak: u32,
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
            && self
                .segment
                .as_ref()
                .is_none_or(|segment| segment_valid(segment, &self.sample_anchor))
            && self.samples.len() <= MAX_ACCEPTED_SAMPLES
            && self
                .samples
                .iter()
                .all(|sample| sample_valid(sample, self.epoch))
            && self
                .pending_high
                .as_ref()
                .is_none_or(|sample| sample_valid(sample, self.epoch))
            && diagnostic_valid(&self.latest_interval)
    }
}

fn anchor_valid(anchor: &QuotaObservationAnchor) -> bool {
    anchor.used_percent.is_finite()
        && anchor.used_percent >= 0.0
        && anchor.reset_at.is_none_or(|value| value >= 0)
        && anchor.telemetry.position.process_id == anchor.telemetry.checkpoint.process_id
}

fn segment_valid(segment: &QuotaSegmentProgress, anchor: &QuotaObservationAnchor) -> bool {
    segment.ended_used_percent.is_finite()
        && segment.ended_used_percent > anchor.used_percent
        && segment.cost_nanos > 0
        && segment.priced_request_count > 0
        && rate_cards_valid(&segment.rate_cards)
}

fn sample_valid(sample: &QuotaCapacitySample, current_epoch: u64) -> bool {
    sample.capacity_credits.is_finite()
        && sample.capacity_credits > 0.0
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
