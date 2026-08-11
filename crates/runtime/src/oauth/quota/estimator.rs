//! Same-process observed dollar-equivalent estimates for upstream quota windows.

use std::{collections::HashMap, time::Duration};

use any2api_domain::OAuthAccountId;
use any2api_provider::api::{OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind};
use tokio::time::Instant;

use super::types::OAuthQuotaUsdEstimate;

const STATE_RETENTION: Duration = Duration::from_secs(3_600);

pub(super) enum CostObservation {
    Priced { usd: f64, rate_card: &'static str },
    Unpriced,
}

#[derive(Default)]
pub(super) struct OAuthQuotaEstimator {
    accounts: HashMap<OAuthAccountId, AccountEstimateState>,
}

struct AccountEstimateState {
    baseline: Vec<WindowBaseline>,
    cumulative_cost_usd: f64,
    unpriced_attempts: u32,
    rate_card: Option<&'static str>,
    last_estimates: Vec<OAuthQuotaUsdEstimate>,
    touched_at: Instant,
}

#[derive(Clone)]
struct WindowBaseline {
    id: String,
    kind: OAuthQuotaWindowKind,
    limit_window_seconds: Option<u64>,
    reset_at: Option<i64>,
    used_percent: f64,
    fetched_at: i64,
    cost_at_baseline_usd: f64,
}

impl OAuthQuotaEstimator {
    pub(super) fn record(
        &mut self,
        id: OAuthAccountId,
        observation: CostObservation,
        now: Instant,
    ) {
        self.prune(now);
        let state = self.accounts.entry(id).or_insert_with(|| empty_state(now));
        state.touched_at = now;
        match observation {
            CostObservation::Priced { usd, rate_card } if usd.is_finite() && usd >= 0.0 => {
                if state.rate_card.is_some_and(|current| current != rate_card) {
                    state.unpriced_attempts = state.unpriced_attempts.saturating_add(1);
                } else {
                    state.rate_card = Some(rate_card);
                    let cumulative = state.cumulative_cost_usd + usd;
                    if cumulative.is_finite() {
                        state.cumulative_cost_usd = cumulative;
                    } else {
                        state.unpriced_attempts = state.unpriced_attempts.saturating_add(1);
                    }
                }
            }
            CostObservation::Priced { .. } | CostObservation::Unpriced => {
                state.unpriced_attempts = state.unpriced_attempts.saturating_add(1);
            }
        }
    }

    pub(super) fn observe_snapshot(
        &mut self,
        id: OAuthAccountId,
        usage: &OAuthQuotaUsage,
        fetched_at: i64,
        now: Instant,
    ) -> Vec<OAuthQuotaUsdEstimate> {
        self.prune(now);
        let state = self.accounts.entry(id).or_insert_with(|| empty_state(now));
        state.touched_at = now;
        let windows = usage
            .rate_limit
            .as_ref()
            .map_or(&[][..], |rate| rate.windows.as_slice());
        if state.unpriced_attempts > 0 {
            state.baseline = windows
                .iter()
                .map(|window| new_baseline(window, fetched_at, state.cumulative_cost_usd))
                .collect();
            state.unpriced_attempts = 0;
            state.rate_card = None;
            state.last_estimates.clear();
            return Vec::new();
        }
        let mut estimates = Vec::with_capacity(windows.len());
        let mut next_baseline = Vec::with_capacity(windows.len());
        for window in windows {
            let baseline = state
                .baseline
                .iter()
                .find(|baseline| same_window(baseline, window));
            let prior = state
                .last_estimates
                .iter()
                .find(|estimate| estimate_matches(estimate, window));
            if let Some(estimate) = baseline
                .filter(|baseline| window.used_percent > baseline.used_percent)
                .and_then(|baseline| {
                    let sample_cost = state.cumulative_cost_usd - baseline.cost_at_baseline_usd;
                    new_estimate(baseline, window, sample_cost, state.rate_card?, fetched_at)
                })
            {
                estimates.push(estimate);
            } else if let Some(estimate) = baseline
                .filter(|baseline| window.used_percent >= baseline.used_percent)
                .and(prior)
                .and_then(|prior| refresh_estimate(prior, window))
            {
                estimates.push(estimate);
            }
            if let Some(baseline) =
                baseline.filter(|baseline| window.used_percent >= baseline.used_percent)
            {
                next_baseline.push(baseline.clone());
            } else {
                next_baseline.push(new_baseline(window, fetched_at, state.cumulative_cost_usd));
            }
        }
        state.baseline = next_baseline;
        state.last_estimates = estimates.clone();
        estimates
    }

    fn prune(&mut self, now: Instant) {
        self.accounts
            .retain(|_, state| now.duration_since(state.touched_at) < STATE_RETENTION);
    }
}

fn empty_state(now: Instant) -> AccountEstimateState {
    AccountEstimateState {
        baseline: Vec::new(),
        cumulative_cost_usd: 0.0,
        unpriced_attempts: 0,
        rate_card: None,
        last_estimates: Vec::new(),
        touched_at: now,
    }
}

fn new_baseline(
    window: &OAuthQuotaWindow,
    fetched_at: i64,
    cumulative_cost_usd: f64,
) -> WindowBaseline {
    WindowBaseline {
        id: window.id.clone(),
        kind: window.kind,
        limit_window_seconds: window.limit_window_seconds,
        reset_at: window.reset_at,
        used_percent: window.used_percent,
        fetched_at,
        cost_at_baseline_usd: cumulative_cost_usd,
    }
}

fn same_window(baseline: &WindowBaseline, window: &OAuthQuotaWindow) -> bool {
    baseline.id == window.id
        && baseline.kind == window.kind
        && baseline.limit_window_seconds == window.limit_window_seconds
        && baseline.reset_at == window.reset_at
}

fn estimate_matches(estimate: &OAuthQuotaUsdEstimate, window: &OAuthQuotaWindow) -> bool {
    estimate.window_id == window.id
        && estimate.window_kind == window.kind
        && estimate.limit_window_seconds == window.limit_window_seconds
        && estimate.window_reset_at == window.reset_at
}

fn new_estimate(
    baseline: &WindowBaseline,
    window: &OAuthQuotaWindow,
    sample_cost_usd: f64,
    rate_card: &str,
    fetched_at: i64,
) -> Option<OAuthQuotaUsdEstimate> {
    let delta = window.used_percent - baseline.used_percent;
    let capacity = sample_cost_usd * 100.0 / delta;
    if !capacity.is_finite() || capacity <= 0.0 {
        return None;
    }
    let used_ratio = window.used_percent.clamp(0.0, 100.0) / 100.0;
    Some(OAuthQuotaUsdEstimate {
        window_id: window.id.clone(),
        window_kind: window.kind,
        limit_window_seconds: window.limit_window_seconds,
        window_reset_at: window.reset_at,
        estimated_capacity_usd: capacity,
        estimated_used_usd: capacity * used_ratio,
        estimated_remaining_usd: capacity * (1.0 - used_ratio),
        sample_cost_usd,
        sample_used_percent_delta: delta,
        sample_started_at: baseline.fetched_at,
        sample_ended_at: fetched_at,
        pricing_basis: rate_card.to_owned(),
    })
}

fn refresh_estimate(
    prior: &OAuthQuotaUsdEstimate,
    window: &OAuthQuotaWindow,
) -> Option<OAuthQuotaUsdEstimate> {
    let capacity = prior.estimated_capacity_usd;
    if !capacity.is_finite() || capacity <= 0.0 {
        return None;
    }
    let used_ratio = window.used_percent.clamp(0.0, 100.0) / 100.0;
    Some(OAuthQuotaUsdEstimate {
        estimated_used_usd: capacity * used_ratio,
        estimated_remaining_usd: capacity * (1.0 - used_ratio),
        ..prior.clone()
    })
}

#[cfg(test)]
mod tests {
    use any2api_provider::api::{OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaWindow};

    use super::*;

    #[test]
    fn infers_capacity_from_complete_cost_and_percent_delta() {
        let id = OAuthAccountId::new();
        let now = Instant::now();
        let mut estimator = OAuthQuotaEstimator::default();
        assert!(
            estimator
                .observe_snapshot(id, &usage(10.0, 2_000), 100, now)
                .is_empty()
        );
        estimator.record(
            id,
            CostObservation::Priced {
                usd: 0.01,
                rate_card: "test_rate_card",
            },
            now,
        );

        let estimates = estimator.observe_snapshot(id, &usage(11.0, 2_000), 400, now);

        assert_eq!(estimates.len(), 1);
        let estimate = &estimates[0];
        assert!((estimate.estimated_capacity_usd - 1.0).abs() < f64::EPSILON);
        assert!((estimate.estimated_remaining_usd - 0.89).abs() < f64::EPSILON);
        assert_eq!(estimate.sample_started_at, 100);
        assert_eq!(estimate.sample_ended_at, 400);
    }

    #[test]
    fn reset_or_unpriced_activity_does_not_create_an_estimate() {
        let id = OAuthAccountId::new();
        let now = Instant::now();
        let mut estimator = OAuthQuotaEstimator::default();
        estimator.observe_snapshot(id, &usage(50.0, 1), 100, now);
        estimator.record(id, CostObservation::Unpriced, now);
        assert!(
            estimator
                .observe_snapshot(id, &usage(51.0, 1), 200, now)
                .is_empty()
        );
        estimator.record(
            id,
            CostObservation::Priced {
                usd: 0.01,
                rate_card: "test_rate_card",
            },
            now,
        );
        assert!(
            estimator
                .observe_snapshot(id, &usage(1.0, 2), 300, now)
                .is_empty()
        );
    }

    #[test]
    fn cumulative_samples_replace_noisy_single_interval_estimates() {
        let id = OAuthAccountId::new();
        let now = Instant::now();
        let mut estimator = OAuthQuotaEstimator::default();
        estimator.observe_snapshot(id, &usage(10.0, 2_000), 100, now);
        estimator.record(
            id,
            CostObservation::Priced {
                usd: 0.3321,
                rate_card: "test_rate_card",
            },
            now,
        );
        let first = estimator.observe_snapshot(id, &usage(11.0, 2_000), 200, now);
        assert!((first[0].estimated_capacity_usd - 33.21).abs() < 1e-12);

        estimator.record(
            id,
            CostObservation::Priced {
                usd: 0.1101,
                rate_card: "test_rate_card",
            },
            now,
        );
        let combined = estimator.observe_snapshot(id, &usage(12.0, 2_000), 300, now);

        assert!((combined[0].sample_cost_usd - 0.4422).abs() < 1e-12);
        assert_eq!(combined[0].sample_used_percent_delta, 2.0);
        assert!((combined[0].estimated_capacity_usd - 22.11).abs() < 1e-12);
    }

    #[test]
    fn unchanged_percentage_does_not_discard_observed_cost() {
        let id = OAuthAccountId::new();
        let now = Instant::now();
        let mut estimator = OAuthQuotaEstimator::default();
        estimator.observe_snapshot(id, &usage(10.0, 2_000), 100, now);
        estimator.record(
            id,
            CostObservation::Priced {
                usd: 0.01,
                rate_card: "test_rate_card",
            },
            now,
        );
        assert!(
            estimator
                .observe_snapshot(id, &usage(10.0, 2_000), 200, now)
                .is_empty()
        );
        estimator.record(
            id,
            CostObservation::Priced {
                usd: 0.02,
                rate_card: "test_rate_card",
            },
            now,
        );

        let estimate = estimator.observe_snapshot(id, &usage(11.0, 2_000), 300, now);

        assert!((estimate[0].sample_cost_usd - 0.03).abs() < f64::EPSILON);
        assert!((estimate[0].estimated_capacity_usd - 3.0).abs() < f64::EPSILON);
        assert_eq!(estimate[0].sample_started_at, 100);
    }

    fn usage(used_percent: f64, reset_at: i64) -> OAuthQuotaUsage {
        OAuthQuotaUsage {
            rate_limit: Some(OAuthQuotaRateLimit {
                allowed: None,
                limit_reached: None,
                windows: vec![OAuthQuotaWindow {
                    id: "primary".to_owned(),
                    kind: OAuthQuotaWindowKind::Time,
                    used_percent,
                    limit_window_seconds: Some(18_000),
                    reset_after_seconds: None,
                    reset_at: Some(reset_at),
                }],
            }),
            credits: None,
            access: None,
            reset_credits: None,
            billing: None,
            token_balance: None,
            subscription_tier: None,
            account_status: None,
        }
    }
}
