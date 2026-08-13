//! Local capacity estimation for OAuth quota windows.
//!
//! Premise (ADR-0144): an OAuth account managed by any2api is consumed
//! exclusively through any2api, so the local RequestLog stream is the
//! complete consumption record. Between two official snapshots,
//! `capacity = local_cost × 100 / Δused%` measures the account's absolute
//! quota capacity directly. Every positive local-cost/official-delta pair
//! contributes to one cumulative ratio.

mod interval;
mod projection;
pub(super) mod state;
mod transition;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaUsage;
use any2api_storage::api::OAuthQuotaEstimationRepository;

use self::state::{QuotaEstimatorState, QuotaWindowKey};
use super::types::OAuthQuotaEstimate;
use crate::request_telemetry::QuotaObservationBoundary;

pub(super) struct OAuthQuotaEstimator {
    repository: Arc<dyn OAuthQuotaEstimationRepository>,
}

pub(super) struct EstimationResult {
    pub(super) state: QuotaEstimatorState,
    pub(super) estimates: Vec<OAuthQuotaEstimate>,
}

impl OAuthQuotaEstimator {
    pub(super) fn new(repository: Arc<dyn OAuthQuotaEstimationRepository>) -> Self {
        Self { repository }
    }

    pub(super) async fn observe(
        &self,
        id: OAuthAccountId,
        usage: &OAuthQuotaUsage,
        previous: Option<QuotaEstimatorState>,
        credential_fingerprint: String,
        expected_unit: QuotaCostUnit,
        observation: QuotaObservationBoundary,
    ) -> EstimationResult {
        let mut state =
            previous.unwrap_or_else(|| QuotaEstimatorState::new(credential_fingerprint.clone()));
        let mut signature_changed = state.credential_fingerprint != credential_fingerprint;
        state.credential_fingerprint = credential_fingerprint;
        // A different subscription tier means a different capacity; missing
        // tier data is not a change.
        if let Some(tier) = usage.subscription_tier.as_deref() {
            if state
                .subscription_tier
                .as_deref()
                .is_some_and(|previous| previous != tier)
            {
                signature_changed = true;
            }
            state.subscription_tier = Some(tier.to_owned());
        }
        if signature_changed {
            state.windows.clear();
        }
        let current_windows = usage
            .rate_limit
            .as_ref()
            .map_or(&[][..], |rate| rate.windows.as_slice());
        let mut next_windows = Vec::with_capacity(current_windows.len());
        for window in current_windows {
            let key = QuotaWindowKey::from_window(window);
            let previous = state.windows.iter().find(|value| value.key == key).cloned();
            let window_state = match previous {
                Some(previous) if !transition::official_reset(&previous, window) => {
                    interval::observe(self, id, window, previous, expected_unit, &observation).await
                }
                Some(previous) => {
                    transition::rollover_window(previous, window, observation.position)
                }
                None => transition::new_window(key, window, observation.position),
            };
            next_windows.push(window_state);
        }
        state.windows = next_windows;
        let estimates = projection::project(usage, Some(&state));
        EstimationResult { state, estimates }
    }
}

pub(in crate::oauth::quota) use projection::project;
