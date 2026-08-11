use any2api_provider::api::{OAuthQuotaWindow, OAuthQuotaWindowKind};
use serde::{Deserialize, Serialize};

use crate::request_telemetry::RequestTelemetryCheckpoint;

use super::super::types::OAuthQuotaIntervalDiagnostic;

const MAX_WINDOWS: usize = 64;
const MAX_SAMPLES: usize = 9;
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
    pub(super) fetched_at_ms: u64,
    pub(super) used_percent: f64,
    pub(super) reset_at: Option<i64>,
    pub(super) checkpoint: RequestTelemetryCheckpoint,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct QuotaCapacitySample {
    pub(super) capacity_credits: f64,
    pub(super) observed_at_ms: u64,
    pub(super) rate_cards: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct QuotaWindowState {
    pub(super) key: QuotaWindowKey,
    pub(super) epoch: u64,
    pub(super) epoch_started_at_ms: u64,
    pub(super) baseline: QuotaObservationAnchor,
    pub(super) samples: Vec<QuotaCapacitySample>,
    pub(super) latest_interval: OAuthQuotaIntervalDiagnostic,
}

impl QuotaWindowState {
    fn valid(&self) -> bool {
        !self.key.id.trim().is_empty()
            && self.key.id.len() <= MAX_SAFE_TEXT_BYTES
            && self.epoch >= 1
            && self.epoch_started_at_ms <= self.baseline.fetched_at_ms
            && self.baseline.used_percent.is_finite()
            && self.baseline.used_percent >= 0.0
            && self.baseline.reset_at.is_none_or(|value| value >= 0)
            && self.samples.len() <= MAX_SAMPLES
            && self.samples.iter().all(|sample| {
                sample.capacity_credits.is_finite()
                    && sample.capacity_credits > 0.0
                    && sample.observed_at_ms >= self.epoch_started_at_ms
                    && sample.rate_cards.len() <= MAX_RATE_CARDS_PER_SAMPLE
                    && sample.rate_cards.iter().all(|rate_card| {
                        !rate_card.trim().is_empty() && rate_card.len() <= MAX_RATE_CARD_BYTES
                    })
            })
            && diagnostic_valid(&self.latest_interval)
    }
}

fn diagnostic_valid(value: &OAuthQuotaIntervalDiagnostic) -> bool {
    value.ended_at >= 0
        && value
            .started_at
            .is_none_or(|started| started >= 0 && started <= value.ended_at)
        && value
            .delta_used_percent
            .is_none_or(|delta| delta.is_finite())
        && value
            .local_cost_credits
            .is_none_or(|cost| cost.is_finite() && cost >= 0.0)
}
