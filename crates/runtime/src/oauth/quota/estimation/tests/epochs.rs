use std::sync::Arc;

use uuid::Uuid;

use super::{
    UsageRepository, checkpoint, checkpoint_for, learned_capacity_state, observe,
    observe_identified, observe_usage, priced, usage, usage_with_tier,
};
use crate::oauth::quota::{
    estimation::OAuthQuotaEstimator,
    types::{OAuthQuotaEstimateConfidence, OAuthQuotaIntervalStatus},
};

/// Scenario F: a large percent drop is a quota reset. The open interval is
/// discarded, but the learned capacity describes the subscription, not the
/// window, and survives as the prior.
#[tokio::test]
async fn official_reset_discards_the_interval_but_keeps_learned_capacity() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = learned_capacity_state(&estimator, &repository, 60.0, None).await;
    let epoch = state.windows[0].epoch;

    // Reset landed between polls; usage had already resumed: 70% → 3%.
    let reset = observe(&estimator, Some(state), 3.0, None, checkpoint(), 2).await;
    let estimate = &reset.estimates[0];
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );
    assert_eq!(estimate.epoch, epoch + 1);
    assert_eq!(estimate.estimated_capacity_credits, Some(1_500.0));
    assert_eq!(estimate.sample_count, 1);
    assert_eq!(estimate.fresh_sample_count, 0);
    state = reset.state;

    // The pre-reset interval is gone: the next sample interval starts at 3%.
    repository.push(priced(105.0));
    let fresh = observe(&estimator, Some(state), 10.0, None, checkpoint(), 3).await;
    assert_eq!(
        fresh.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(fresh.estimates[0].sample_count, 2);
    assert_eq!(fresh.estimates[0].fresh_sample_count, 1);
    assert_eq!(fresh.estimates[0].estimated_capacity_credits, Some(1_500.0));
    let queries = repository.queries();
    assert_eq!(queries.last().unwrap().0.sequence, 2);
}

/// Scenario G: a normal window rollover (new reset_at identity) re-anchors
/// usage but inherits the capacity prior into the new epoch.
#[tokio::test]
async fn window_rollover_inherits_the_capacity_prior() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let state = learned_capacity_state(&estimator, &repository, 40.0, Some(1_000)).await;

    let rolled = observe(&estimator, Some(state), 5.0, Some(19_000), checkpoint(), 2).await;
    let estimate = &rolled.estimates[0];
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );
    assert_eq!(estimate.estimated_capacity_credits, Some(1_500.0));
    assert_eq!(estimate.fresh_sample_count, 0);
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Learning);
}

/// A reset_at drift within the 60-second jitter tolerance is the same window:
/// no rollover, the open interval keeps accumulating.
#[tokio::test]
async fn one_minute_reset_timestamp_drift_keeps_the_epoch() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let state = observe(&estimator, None, 10.0, Some(1_000), checkpoint(), 0)
        .await
        .state;
    let epoch = state.windows[0].epoch;

    repository.push(priced(150.0));
    let result = observe(&estimator, Some(state), 20.0, Some(1_045), checkpoint(), 1).await;
    assert_eq!(result.estimates[0].epoch, epoch);
    assert_eq!(
        result.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
}

/// Scenario H: a credential identity change is a capacity-signature change —
/// the learned samples describe a different account and are dropped.
#[tokio::test]
async fn credential_identity_change_restarts_capacity_learning() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let state = learned_capacity_state(&estimator, &repository, 10.0, None).await;

    let switched = observe_identified(
        &estimator,
        Some(state),
        usage(25.0, None),
        "identity-b",
        checkpoint(),
        2,
    )
    .await;
    let estimate = &switched.estimates[0];
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );
    assert!(estimate.estimated_capacity_credits.is_none());
    assert_eq!(estimate.sample_count, 0);
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Unknown);
}

/// Scenario H: a subscription tier change means a different absolute
/// capacity; learning restarts. Gaining tier data for the first time is not a
/// change.
#[tokio::test]
async fn subscription_tier_change_restarts_capacity_learning() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    // Tier appears: keep the open interval and learn from it.
    repository.push(priced(150.0));
    let appeared = observe_usage(
        &estimator,
        Some(state),
        usage_with_tier(20.0, None, "plus"),
        checkpoint(),
        1,
    )
    .await;
    assert_eq!(
        appeared.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
    state = appeared.state;

    // Tier changes: the learned capacity no longer applies.
    let upgraded = observe_usage(
        &estimator,
        Some(state),
        usage_with_tier(22.0, None, "pro"),
        checkpoint(),
        2,
    )
    .await;
    assert_eq!(upgraded.estimates[0].sample_count, 0);
    assert!(upgraded.estimates[0].estimated_capacity_credits.is_none());
    assert_eq!(
        upgraded.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Unknown
    );
}

/// Scenario H: a window structure change (different duration) is a different
/// quota shape; its samples do not carry over.
#[tokio::test]
async fn changed_window_duration_starts_clean() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let state = learned_capacity_state(&estimator, &repository, 10.0, None).await;

    let mut reshaped_window = super::window(25.0, None);
    reshaped_window.limit_window_seconds = Some(10_800);
    let reshaped = observe_usage(
        &estimator,
        Some(state),
        super::usage_for_window(reshaped_window),
        checkpoint(),
        2,
    )
    .await;
    assert_eq!(reshaped.estimates[0].sample_count, 0);
    assert!(reshaped.estimates[0].estimated_capacity_credits.is_none());
}

/// A process restart breaks the sequence fence: the running interval fails
/// closed, the capacity prior survives, and the next same-process interval
/// learns normally.
#[tokio::test]
async fn restart_keeps_the_prior_and_recovers_with_the_next_clean_interval() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = learned_capacity_state(&estimator, &repository, 10.0, Some(1_000)).await;

    let successor = checkpoint_for(Uuid::from_u128(7));
    let restarted = observe(
        &estimator,
        Some(state),
        24.0,
        Some(1_000),
        successor.clone(),
        0,
    )
    .await;
    let estimate = &restarted.estimates[0];
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    assert_eq!(estimate.estimated_capacity_credits, Some(1_500.0));
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Degraded);
    state = restarted.state;

    repository.push(priced(90.0));
    let recovered = observe(&estimator, Some(state), 30.0, Some(1_000), successor, 1).await;
    assert_eq!(
        recovered.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(recovered.estimates[0].sample_count, 2);
}
