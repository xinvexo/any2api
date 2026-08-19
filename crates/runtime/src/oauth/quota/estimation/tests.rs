use std::sync::Mutex;

use any2api_domain::{OAuthAccountId, QuotaCostUnit, RequestTelemetryPosition};
use any2api_provider::api::{
    OAuthQuotaRateLimit, OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
};
use any2api_storage::api::{OAuthQuotaEstimationRepository, StorageError};
use async_trait::async_trait;
use uuid::Uuid;

use super::{OAuthQuotaEstimator, state::QuotaEstimatorState};
use crate::request_telemetry::QuotaObservationBoundary;

const RESET_AT: i64 = 20_000;
const WINDOW_SECONDS: u64 = 18_000;
const WINDOW_STARTED_AT_MS: u64 = 2_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Query {
    window_started_at_ms: u64,
    observed_at_ms: u64,
    observation_position: RequestTelemetryPosition,
}

#[derive(Default)]
struct UsageRepository {
    cost_nanos: Mutex<u64>,
    queries: Mutex<Vec<Query>>,
}

impl UsageRepository {
    fn set_cost(&self, credits: f64) {
        *self.cost_nanos.lock().expect("usage cost") = (credits * 1_000_000_000.0) as u64;
    }

    fn queries(&self) -> Vec<Query> {
        self.queries.lock().expect("usage queries").clone()
    }
}

#[async_trait]
impl OAuthQuotaEstimationRepository for UsageRepository {
    async fn oauth_quota_window_local_cost_nanos(
        &self,
        _id: OAuthAccountId,
        window_started_at_ms: u64,
        observed_at_ms: u64,
        observation_position: RequestTelemetryPosition,
        _unit: QuotaCostUnit,
    ) -> Result<u64, StorageError> {
        self.queries.lock().expect("usage queries").push(Query {
            window_started_at_ms,
            observed_at_ms,
            observation_position,
        });
        Ok(*self.cost_nanos.lock().expect("usage cost"))
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
                limit_window_seconds: Some(WINDOW_SECONDS),
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
        observed_at_ms: 5_000_000 + sequence * 1_000,
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
        .expect("whole-cycle estimate")
}

#[tokio::test]
async fn refresh_sums_the_whole_official_cycle() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(4.0);
    let estimator = OAuthQuotaEstimator::new(repository.clone());

    let result = observe(&estimator, None, 10.0, Some(RESET_AT), 7).await;

    assert_eq!(result.estimates[0].estimated_used_credits, Some(4.0));
    assert_eq!(result.estimates[0].estimated_capacity_credits, Some(40.0));
    assert_eq!(
        repository.queries(),
        vec![Query {
            window_started_at_ms: WINDOW_STARTED_AT_MS,
            observed_at_ms: observation(7).observed_at_ms,
            observation_position: observation(7).position,
        }]
    );
}

#[tokio::test]
async fn later_refresh_replaces_the_whole_cycle_total_instead_of_adding_segments() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(4.0);
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let first = observe(&estimator, None, 10.0, Some(RESET_AT), 1).await;

    repository.set_cost(10.0);
    let second = observe(&estimator, Some(first.state), 20.0, Some(RESET_AT), 2).await;

    assert_eq!(second.estimates[0].estimated_used_credits, Some(10.0));
    assert_eq!(second.estimates[0].estimated_capacity_credits, Some(50.0));
    assert_eq!(repository.queries().len(), 2);
    assert!(
        repository
            .queries()
            .iter()
            .all(|query| query.window_started_at_ms == WINDOW_STARTED_AT_MS)
    );
}

#[tokio::test]
async fn capacity_waits_for_two_percent_but_local_usage_is_visible() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(1.6);
    let estimator = OAuthQuotaEstimator::new(repository);

    let early = observe(&estimator, None, 1.9, Some(RESET_AT), 1).await;
    assert_eq!(early.estimates[0].estimated_used_credits, Some(1.6));
    assert_eq!(early.estimates[0].estimated_capacity_credits, None);
    assert_eq!(early.estimates[0].estimated_remaining_credits, None);

    let boundary = observe(&estimator, Some(early.state), 2.0, Some(RESET_AT), 2).await;
    assert_eq!(boundary.estimates[0].estimated_capacity_credits, Some(80.0));
}

#[tokio::test]
async fn official_reset_uses_only_the_new_cycle_total() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(36.0);
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let old = observe(&estimator, None, 90.0, Some(RESET_AT), 1).await;

    repository.set_cost(2.0);
    let new_reset = RESET_AT + i64::try_from(WINDOW_SECONDS).expect("window seconds");
    let reset = observe(&estimator, Some(old.state), 5.0, Some(new_reset), 15_001).await;

    assert_eq!(reset.estimates[0].estimated_used_credits, Some(2.0));
    assert_eq!(reset.estimates[0].estimated_capacity_credits, Some(40.0));
    assert_eq!(repository.queries().len(), 2);
    assert_eq!(
        repository
            .queries()
            .last()
            .expect("new cycle query")
            .window_started_at_ms,
        u64::try_from(RESET_AT).expect("reset") * 1_000
    );
}

#[tokio::test]
async fn reset_credit_cycle_that_starts_after_observation_is_empty() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(99.0);
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let future_reset = i64::try_from(observation(1).observed_at_ms / 1_000)
        .expect("observation seconds")
        + i64::try_from(WINDOW_SECONDS).expect("window seconds")
        + 60;

    let result = observe(&estimator, None, 0.0, Some(future_reset), 1).await;

    assert_eq!(result.estimates[0].estimated_used_credits, Some(0.0));
    assert_eq!(result.estimates[0].estimated_capacity_credits, None);
    assert!(repository.queries().is_empty());
}

#[tokio::test]
async fn identity_change_keeps_local_usage_but_blocks_current_cycle_capacity() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(4.0);
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let first = observe(&estimator, None, 10.0, Some(RESET_AT), 1).await;

    let changed = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(20.0, Some(RESET_AT), Some("pro")),
            Some(first.state),
            "identity-b".into(),
            QuotaCostUnit::CodexCredits,
            observation(2),
        )
        .await
        .expect("identity transition");

    assert_eq!(changed.estimates[0].estimated_used_credits, Some(4.0));
    assert_eq!(changed.estimates[0].estimated_capacity_credits, None);
    assert_eq!(repository.queries().len(), 2);
}

#[tokio::test]
async fn missing_rate_limit_does_not_discard_the_cycle_state() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    repository.set_cost(4.0);
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 10.0, Some(RESET_AT), 1).await;
    let mut missing = usage(10.0, Some(RESET_AT), None);
    missing.rate_limit = None;

    let absent = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &missing,
            Some(first.state),
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            observation(2),
        )
        .await
        .expect("missing rate limit");

    assert!(absent.estimates.is_empty());
    assert_eq!(absent.state.windows.len(), 1);
}

#[tokio::test]
async fn windows_without_a_complete_official_cycle_are_not_estimated() {
    let repository = std::sync::Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(repository.clone());

    let result = observe(&estimator, None, 10.0, None, 1).await;

    assert!(result.estimates.is_empty());
    assert!(result.state.windows.is_empty());
    assert!(repository.queries().is_empty());
}
