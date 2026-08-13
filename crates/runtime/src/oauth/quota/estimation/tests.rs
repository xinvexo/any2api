use std::{collections::VecDeque, sync::Mutex};

use any2api_domain::{OAuthAccountId, QuotaCostUnit, RequestTelemetryPosition};
use any2api_provider::api::{
    OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
};
use any2api_storage::api::{OAuthQuotaEstimationRepository, StorageError};
use async_trait::async_trait;
use uuid::Uuid;

use super::{OAuthQuotaEstimator, state::QuotaEstimatorState};
use crate::request_telemetry::QuotaObservationBoundary;

#[derive(Default)]
struct UsageRepository {
    costs: Mutex<VecDeque<Result<u64, StorageError>>>,
}

impl UsageRepository {
    fn push_cost(&self, credits: f64) {
        self.costs
            .lock()
            .expect("usage costs")
            .push_back(Ok((credits * 1_000_000_000.0) as u64));
    }
}

#[async_trait]
impl OAuthQuotaEstimationRepository for UsageRepository {
    async fn oauth_quota_local_cost_nanos(
        &self,
        _id: OAuthAccountId,
        _interval_start: RequestTelemetryPosition,
        _interval_end: RequestTelemetryPosition,
        _unit: QuotaCostUnit,
    ) -> Result<u64, StorageError> {
        self.costs
            .lock()
            .expect("usage costs")
            .pop_front()
            .unwrap_or(Ok(100_000_000_000))
    }
}

fn usage(percent: f64, reset_at: Option<i64>, tier: Option<&str>) -> OAuthQuotaUsage {
    OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: Some(true),
            limit_reached: Some(false),
            windows: vec![OAuthQuotaWindow {
                id: "primary".into(),
                kind: OAuthQuotaWindowKind::Time,
                used_percent: percent,
                limit_window_seconds: Some(18_000),
                reset_after_seconds: None,
                reset_at,
            }],
        }),
        credits: None,
        access: None,
        reset_credits: None,
        billing: None,
        token_balance: None,
        subscription_tier: tier.map(str::to_owned),
        account_status: None,
    }
}

fn observation(sequence: u64) -> QuotaObservationBoundary {
    QuotaObservationBoundary {
        observed_at_ms: sequence * 1_000,
        position: RequestTelemetryPosition {
            process_id: Uuid::nil(),
            sequence,
        },
    }
}

async fn observe(
    estimator: &OAuthQuotaEstimator,
    state: Option<QuotaEstimatorState>,
    percent: f64,
    reset_at: Option<i64>,
    sequence: u64,
) -> super::EstimationResult {
    estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(percent, reset_at, None),
            state,
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            observation(sequence),
        )
        .await
}

#[tokio::test]
async fn first_local_interval_sets_capacity() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.push_cost(10.0);
    let estimator = OAuthQuotaEstimator::new(repository);
    let baseline = observe(&estimator, None, 10.0, Some(100), 1).await;
    let result = observe(&estimator, Some(baseline.state), 20.0, Some(100), 2).await;
    assert_eq!(result.estimates[0].completed_interval_count, 1);
    assert_eq!(result.estimates[0].estimated_capacity_credits, Some(100.0));
}

#[tokio::test]
async fn every_recorded_interval_remains_in_cumulative_ratio() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.push_cost(10.0);
    repository.push_cost(20.0);
    let estimator = OAuthQuotaEstimator::new(repository);
    let baseline = observe(&estimator, None, 0.0, Some(100), 1).await;
    let first = observe(&estimator, Some(baseline.state), 10.0, Some(100), 2).await;
    let second = observe(&estimator, Some(first.state), 20.0, Some(100), 3).await;
    assert_eq!(second.estimates[0].completed_interval_count, 2);
    assert_eq!(second.estimates[0].estimated_capacity_credits, Some(150.0));
}

#[tokio::test]
async fn official_reset_reanchors_and_keeps_all_prior_totals() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.push_cost(10.0);
    repository.push_cost(20.0);
    let estimator = OAuthQuotaEstimator::new(repository);
    let baseline = observe(&estimator, None, 80.0, Some(100), 1).await;
    let first = observe(&estimator, Some(baseline.state), 90.0, Some(100), 2).await;
    let reset = observe(&estimator, Some(first.state), 2.0, Some(200), 3).await;
    assert_eq!(reset.estimates[0].completed_interval_count, 1);
    assert_eq!(reset.estimates[0].estimated_capacity_credits, Some(100.0));
    let after_reset = observe(&estimator, Some(reset.state), 12.0, Some(200), 4).await;
    assert_eq!(after_reset.estimates[0].completed_interval_count, 2);
    assert_eq!(
        after_reset.estimates[0].estimated_capacity_credits,
        Some(150.0)
    );
}

#[tokio::test]
async fn identity_change_discards_old_capacity() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.push_cost(10.0);
    let estimator = OAuthQuotaEstimator::new(repository);
    let baseline = observe(&estimator, None, 10.0, Some(100), 1).await;
    let first = observe(&estimator, Some(baseline.state), 20.0, Some(100), 2).await;
    let changed = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(30.0, Some(100), Some("pro")),
            Some(first.state),
            "identity-b".into(),
            QuotaCostUnit::CodexCredits,
            observation(3),
        )
        .await;
    assert_eq!(changed.estimates[0].completed_interval_count, 0);
    assert_eq!(changed.estimates[0].estimated_capacity_credits, None);
}

#[tokio::test]
async fn missing_local_cost_keeps_interval_open() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository
        .costs
        .lock()
        .expect("usage costs")
        .push_back(Ok(0));
    repository.push_cost(10.0);
    let estimator = OAuthQuotaEstimator::new(repository);
    let baseline = observe(&estimator, None, 0.0, Some(100), 1).await;
    let waiting = observe(&estimator, Some(baseline.state), 10.0, Some(100), 2).await;
    assert_eq!(waiting.estimates[0].completed_interval_count, 0);
    let completed = observe(&estimator, Some(waiting.state), 20.0, Some(100), 3).await;
    assert_eq!(completed.estimates[0].completed_interval_count, 1);
    assert_eq!(
        completed.estimates[0].estimated_capacity_credits,
        Some(50.0)
    );
}
