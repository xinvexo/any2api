//! Dollar-equivalent quota estimates derived from retained RequestLog usage.

use std::sync::Arc;

use any2api_domain::{OAuthAccountId, TokenUsage};
use any2api_provider::api::{OAuthQuotaUsage, OAuthQuotaWindow, ProviderDriver};
use any2api_storage::api::{
    OAuthQuotaEstimationRepository, OAuthQuotaRequestLogUsage, StorageError,
};

use super::types::{OAuthQuotaSnapshot, OAuthQuotaUsdEstimate};

const MILLISECONDS_PER_SECOND: u64 = 1_000;

pub(super) struct OAuthQuotaEstimator {
    repository: Arc<dyn OAuthQuotaEstimationRepository>,
}

impl OAuthQuotaEstimator {
    pub(super) fn new(repository: Arc<dyn OAuthQuotaEstimationRepository>) -> Self {
        Self { repository }
    }

    pub(super) async fn estimate(
        &self,
        id: OAuthAccountId,
        usage: &OAuthQuotaUsage,
        previous: Option<&OAuthQuotaSnapshot>,
        driver: &dyn ProviderDriver,
        fetched_at_ms: u64,
    ) -> Result<Vec<OAuthQuotaUsdEstimate>, StorageError> {
        let local_reset_at_ms = self.repository.load_oauth_quota_reset_boundary(id).await?;
        let windows = usage
            .rate_limit
            .as_ref()
            .map_or(&[][..], |rate| rate.windows.as_slice());
        let credits_usable = usage
            .credits
            .as_ref()
            .is_some_and(|credits| credits.usable());
        let mut estimates = Vec::with_capacity(windows.len());
        for window in windows {
            let Some((started_at_ms, ended_at_ms)) =
                window_bounds(window, fetched_at_ms, local_reset_at_ms)
            else {
                continue;
            };
            if window.used_percent <= 0.0 || !window.used_percent.is_finite() {
                continue;
            }
            if credits_usable && window.used_percent >= 100.0 {
                if let Some(prior) =
                    previous.and_then(|snapshot| prior_estimate(snapshot, window, started_at_ms))
                {
                    estimates.push(prior.clone());
                }
                continue;
            }
            let log_usage = self
                .repository
                .oauth_quota_request_log_usage(id, started_at_ms, ended_at_ms)
                .await?;
            if let Some(estimate) =
                estimate_from_logs(driver, window, &log_usage, started_at_ms, ended_at_ms)
            {
                estimates.push(estimate);
            }
        }
        Ok(estimates)
    }

    pub(super) async fn record_reset(
        &self,
        id: OAuthAccountId,
        reset_at_ms: u64,
    ) -> Result<(), StorageError> {
        self.repository
            .record_oauth_quota_reset(id, reset_at_ms)
            .await
    }
}

pub(super) fn window_bounds(
    window: &OAuthQuotaWindow,
    fetched_at_ms: u64,
    local_reset_at_ms: Option<u64>,
) -> Option<(u64, u64)> {
    let duration_ms = window
        .limit_window_seconds?
        .checked_mul(MILLISECONDS_PER_SECOND)?;
    let started_at_ms = match window.reset_at {
        Some(reset_at) => seconds_to_millis(reset_at)?.checked_sub(duration_ms)?,
        None => fetched_at_ms.checked_sub(duration_ms)?,
    }
    .max(local_reset_at_ms.unwrap_or_default());
    let ended_at_ms = fetched_at_ms;
    (started_at_ms < ended_at_ms).then_some((started_at_ms, ended_at_ms))
}

fn seconds_to_millis(seconds: i64) -> Option<u64> {
    u64::try_from(seconds)
        .ok()?
        .checked_mul(MILLISECONDS_PER_SECOND)
}

pub(super) fn estimate_from_logs(
    driver: &dyn ProviderDriver,
    window: &OAuthQuotaWindow,
    usage: &OAuthQuotaRequestLogUsage,
    started_at_ms: u64,
    ended_at_ms: u64,
) -> Option<OAuthQuotaUsdEstimate> {
    if usage.models.is_empty() {
        return None;
    }
    let mut pricing_basis = None;
    let mut cost = 0.0_f64;
    for model in &usage.models {
        let rate = driver.oauth_quota_cost_rate(&model.public_model)?;
        if pricing_basis.is_some_and(|current| current != rate.rate_card()) {
            return None;
        }
        pricing_basis = Some(rate.rate_card());
        let model_cost = rate.estimate_usd(TokenUsage::new(
            Some(model.input_tokens),
            Some(model.output_tokens),
            Some(model.cache_read_tokens),
        ))?;
        cost += model_cost;
    }
    let used_percent = window.used_percent.clamp(0.0, 100.0);
    let capacity = cost * 100.0 / used_percent;
    if !cost.is_finite() || cost <= 0.0 || !capacity.is_finite() || capacity <= 0.0 {
        return None;
    }
    Some(OAuthQuotaUsdEstimate {
        window_id: window.id.clone(),
        window_kind: window.kind,
        limit_window_seconds: window.limit_window_seconds,
        window_reset_at: window.reset_at,
        estimated_capacity_usd: capacity,
        estimated_used_usd: cost,
        estimated_remaining_usd: (capacity - cost).max(0.0),
        sample_cost_usd: cost,
        sample_used_percent: used_percent,
        sample_started_at: i64::try_from(started_at_ms / 1_000).ok()?,
        sample_ended_at: i64::try_from(ended_at_ms / 1_000).ok()?,
        unpriced_request_count: usage.unpriced_request_count,
        pricing_basis: pricing_basis?.to_owned(),
    })
}

pub(super) fn prior_estimate<'a>(
    snapshot: &'a OAuthQuotaSnapshot,
    window: &OAuthQuotaWindow,
    started_at_ms: u64,
) -> Option<&'a OAuthQuotaUsdEstimate> {
    snapshot.usd_estimates.iter().find(|estimate| {
        estimate.window_id == window.id
            && estimate.window_kind == window.kind
            && estimate.limit_window_seconds == window.limit_window_seconds
            && estimate.window_reset_at == window.reset_at
            && seconds_to_millis(estimate.sample_started_at).is_some_and(|sample_start| {
                sample_start.saturating_add(MILLISECONDS_PER_SECOND - 1) >= started_at_ms
            })
    })
}
