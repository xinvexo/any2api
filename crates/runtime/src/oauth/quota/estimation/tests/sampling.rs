use std::sync::Arc;

use super::{
    UsageRepository, checkpoint, costless, learned_capacity_state, observe, priced,
    pruned_checkpoint, telemetry_observation, unpriced,
};
use crate::oauth::quota::{
    estimation::OAuthQuotaEstimator,
    types::{OAuthQuotaEstimateConfidence, OAuthQuotaIntervalStatus},
};

/// Scenario A: `used 10% → 20%` with 37.5 credits ($1.50) of local cost
/// calibrates the account to 375 credits ($15) of absolute capacity.
#[tokio::test]
async fn one_clean_interval_calibrates_absolute_capacity() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);

    let baseline = observe(&estimator, None, 10.0, None, checkpoint(), 0).await;
    assert_eq!(
        baseline.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::AwaitingBaseline
    );
    assert_eq!(
        baseline.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Unknown
    );

    repository.push(priced(37.5));
    let result = observe(
        &estimator,
        Some(baseline.state),
        20.0,
        None,
        checkpoint(),
        1,
    )
    .await;

    let estimate = &result.estimates[0];
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(estimate.estimated_capacity_credits, Some(375.0));
    assert_eq!(estimate.estimated_used_credits, Some(75.0));
    assert_eq!(estimate.estimated_remaining_credits, Some(300.0));
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Learning);
    assert_eq!(estimate.sample_count, 1);
    assert_eq!(estimate.rate_cards, vec![super::RATE_CARD.to_owned()]);
}

/// Scenario B: repeated clean intervals converge on the true capacity.
#[tokio::test]
async fn repeated_clean_samples_converge_on_the_true_capacity() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    let capacities = [1_520.0, 1_490.0, 1_500.0, 1_510.0, 1_480.0];
    for (index, capacity) in capacities.iter().enumerate() {
        let percent = 10.0 + 5.0 * (index as f64 + 1.0);
        repository.push(priced(capacity * 5.0 / 100.0));
        let result = observe(
            &estimator,
            Some(state),
            percent,
            None,
            checkpoint(),
            index + 1,
        )
        .await;
        assert_eq!(
            result.estimates[0].latest_interval.status,
            OAuthQuotaIntervalStatus::ValidSample
        );
        state = result.state;
    }

    let result = observe(&estimator, Some(state), 35.2, None, checkpoint(), 7).await;
    let estimate = &result.estimates[0];
    assert_eq!(estimate.estimated_capacity_credits, Some(1_500.0));
    assert_eq!(estimate.sample_count, 5);
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Stable);
}

/// Scenario C: the estimate is a measurement, not a lower bound — later
/// samples below the first one pull it down.
#[tokio::test]
async fn estimate_moves_down_when_later_samples_measure_lower() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    let capacities = [1_550.0, 1_500.0, 1_490.0, 1_510.0];
    for (index, capacity) in capacities.iter().enumerate() {
        let percent = 10.0 + 5.0 * (index as f64 + 1.0);
        repository.push(priced(capacity * 5.0 / 100.0));
        state = observe(
            &estimator,
            Some(state),
            percent,
            None,
            checkpoint(),
            index + 1,
        )
        .await
        .state;
    }

    let result = observe(&estimator, Some(state), 30.2, None, checkpoint(), 6).await;
    let capacity = result.estimates[0]
        .estimated_capacity_credits
        .expect("capacity");
    assert_eq!(capacity, 1_505.0);
    assert!(capacity < 1_550.0);
}

/// Scenario L: samples with larger official deltas carry more weight, so a
/// small-delta bootstrap outlier cannot hold the estimate away from the
/// large-delta measurements.
#[tokio::test]
async fn larger_delta_samples_dominate_the_aggregate() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    // Bootstrap sample: Δ3% at capacity 1600.
    repository.push(priced(48.0));
    state = observe(&estimator, Some(state), 13.0, None, checkpoint(), 1)
        .await
        .state;
    // Δ10% at capacity 1500.
    repository.push(priced(150.0));
    state = observe(&estimator, Some(state), 23.0, None, checkpoint(), 2)
        .await
        .state;
    // Δ5% at capacity 1620.
    repository.push(priced(81.0));
    let result = observe(&estimator, Some(state), 28.0, None, checkpoint(), 3).await;

    // Plain median would report 1600; the Δ-weighted median follows the
    // heavier 10-point sample.
    assert_eq!(
        result.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
}

/// An official delta whose local cost has not landed yet (requests still in
/// flight, upstream accounting lag) keeps accumulating instead of splitting
/// the cost from its percent: the interval telescopes to a correct sample.
#[tokio::test]
async fn costless_official_delta_waits_for_the_local_cost_to_land() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    repository.push(costless());
    let waiting = observe(&estimator, Some(state), 13.0, None, checkpoint(), 1).await;
    assert_eq!(
        waiting.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    assert!(waiting.estimates[0].estimated_capacity_credits.is_none());
    state = waiting.state;

    repository.push(priced(90.0));
    let result = observe(&estimator, Some(state), 16.0, None, checkpoint(), 2).await;
    assert_eq!(
        result.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    // Δ = 16 − 10 over the held anchor, not 16 − 13.
    assert_eq!(
        result.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
}

/// Unpriced requests poison the whole interval: no sample, fresh anchor,
/// degraded confidence until a clean interval closes.
#[tokio::test]
async fn unpriced_usage_discards_the_interval_without_learning() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = learned_capacity_state(&estimator, &repository, 10.0, None).await;

    repository.push(unpriced());
    let poisoned = observe(&estimator, Some(state), 30.0, None, checkpoint(), 2).await;
    assert_eq!(
        poisoned.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::UnpricedUsage
    );
    assert_eq!(
        poisoned.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Degraded
    );
    assert_eq!(poisoned.estimates[0].sample_count, 1);
    state = poisoned.state;

    // The next interval starts at the poisoned observation, not before it.
    repository.push(priced(75.0));
    let recovered = observe(&estimator, Some(state), 35.0, None, checkpoint(), 3).await;
    assert_eq!(
        recovered.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(recovered.estimates[0].sample_count, 2);
    let queries = repository.queries();
    assert_eq!(queries.last().unwrap().0.sequence, 2);
}

#[tokio::test]
async fn interval_query_failure_discards_the_interval() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    repository.push_error();
    let result = observe(&estimator, Some(state), 20.0, None, checkpoint(), 1).await;
    assert_eq!(
        result.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Invalid
    );
    assert!(result.estimates[0].estimated_capacity_credits.is_none());
}

/// Interval membership is decided by the monotonic sequence fence, not the
/// wall clock: a clock rollback between observations does not corrupt the
/// sample.
#[tokio::test]
async fn wall_clock_rollback_does_not_change_sequence_interval_membership() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let account = any2api_domain::OAuthAccountId::from_uuid(uuid::Uuid::nil());

    let baseline = estimator
        .observe(
            account,
            &super::usage(10.0, None),
            None,
            "identity-a".into(),
            any2api_domain::QuotaCostUnit::CodexCredits,
            telemetry_observation(checkpoint(), 10_000, 4),
            None,
        )
        .await;
    repository.push(priced(150.0));
    let result = estimator
        .observe(
            account,
            &super::usage(20.0, None),
            Some(baseline.state),
            "identity-a".into(),
            any2api_domain::QuotaCostUnit::CodexCredits,
            telemetry_observation(checkpoint(), 5_000, 9),
            None,
        )
        .await;

    assert_eq!(
        result.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(
        result.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
    let queries = repository.queries();
    assert_eq!(queries[0].0.sequence, 4);
    assert_eq!(queries[0].1.sequence, 9);
}

/// Retention before the anchor is harmless, but pruning into the open interval
/// fails closed even when the interval query itself succeeded.
#[tokio::test]
async fn pruning_only_invalidates_intervals_it_reaches_into() {
    for (pruned_through, status, interval_pruned, capacity) in [
        (
            2,
            OAuthQuotaIntervalStatus::ValidSample,
            false,
            Some(1_500.0),
        ),
        (3, OAuthQuotaIntervalStatus::TelemetryIncomplete, true, None),
    ] {
        let repository = Arc::new(UsageRepository::default());
        let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
        let state = observe(&estimator, None, 10.0, None, checkpoint(), 2)
            .await
            .state;
        repository.push(priced(150.0));

        let result = observe(
            &estimator,
            Some(state),
            20.0,
            None,
            pruned_checkpoint(pruned_through),
            4,
        )
        .await;
        let estimate = &result.estimates[0];
        assert_eq!(estimate.latest_interval.status, status);
        assert_eq!(estimate.latest_interval.interval_pruned, interval_pruned);
        assert_eq!(estimate.estimated_capacity_credits, capacity);
    }
}
