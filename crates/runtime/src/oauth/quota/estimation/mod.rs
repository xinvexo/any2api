//! Local capacity estimation for OAuth quota windows.
//!
//! Premise: an OAuth account managed by any2api is consumed through
//! any2api, so the current official quota cycle can be measured by summing
//! the account's persisted RequestLogs. Each refresh recomputes that whole
//! cycle; adjacent refreshes are never treated as independent samples.

mod cycle;
mod projection;
pub(super) mod state;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use any2api_domain::{OAuthAccountId, QuotaCostUnit};
use any2api_provider::api::OAuthQuotaUsage;
use any2api_storage::api::{OAuthQuotaEstimationRepository, StorageError};

use self::state::{
    MAX_WINDOWS, OfficialQuotaCycle, QuotaEstimatorState, QuotaWindowKey, QuotaWindowState,
};
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
    ) -> Result<EstimationResult, StorageError> {
        let had_previous = previous.is_some();
        let mut state =
            previous.unwrap_or_else(|| QuotaEstimatorState::new(credential_fingerprint.clone()));
        let mut signature_changed =
            had_previous && state.credential_fingerprint != credential_fingerprint;
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
            state
                .windows
                .iter_mut()
                .for_each(QuotaWindowState::block_capacity);
        }
        let current_windows = usage
            .rate_limit
            .as_ref()
            .map_or(&[][..], |rate| rate.windows.as_slice());
        let mut next_windows = Vec::with_capacity(current_windows.len().min(MAX_WINDOWS));
        for window in current_windows.iter().take(MAX_WINDOWS) {
            let key = QuotaWindowKey::from_window(window);
            let Some(cycle) = OfficialQuotaCycle::from_window(window) else {
                continue;
            };
            let previous = state.windows.iter().find(|value| value.key == key).cloned();
            let capacity_eligible = !signature_changed
                && previous
                    .as_ref()
                    .is_none_or(|state| !state.matches_cycle(cycle) || state.capacity_eligible);
            let window_state = cycle::measure(
                self,
                id,
                key,
                cycle,
                capacity_eligible,
                expected_unit,
                &observation,
            )
            .await?;
            next_windows.push(window_state);
        }
        for previous in state.windows {
            if next_windows.len() == MAX_WINDOWS {
                break;
            }
            if next_windows.iter().all(|window| window.key != previous.key) {
                next_windows.push(previous);
            }
        }
        state.windows = next_windows;
        let estimates = projection::project(usage, Some(&state));
        Ok(EstimationResult { state, estimates })
    }
}

pub(in crate::oauth::quota) use projection::project;
