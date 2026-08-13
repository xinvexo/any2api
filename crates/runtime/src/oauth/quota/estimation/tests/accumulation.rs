use std::sync::Arc;

use super::{
    UsageRepository, checkpoint, learned_capacity_state, observe, priced, queue_dropped_checkpoint,
    storage_failed_checkpoint,
};
use crate::oauth::quota::{
    estimation::OAuthQuotaEstimator,
    types::{OAuthQuotaEstimateConfidence, OAuthQuotaIntervalStatus},
};

/// Scenario D: sub-threshold percent changes hold the sample anchor until the
/// accumulated delta is large enough for one well-conditioned sample.
#[tokio::test]
async fn small_deltas_accumulate_into_one_sample_from_the_original_anchor() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    for (index, percent) in [10.2, 10.4].into_iter().enumerate() {
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
            OAuthQuotaIntervalStatus::NoChange
        );
        state = result.state;
    }
    // 0.5 ≤ Δ < mint threshold: the interval is probed and stays open.
    repository.push(priced(15.0));
    let probed = observe(&estimator, Some(state), 11.0, None, checkpoint(), 3).await;
    assert_eq!(
        probed.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    state = probed.state;

    repository.push(priced(78.0));
    let minted = observe(&estimator, Some(state), 15.2, None, checkpoint(), 4).await;
    assert_eq!(
        minted.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    // Δ = 15.2 − 10.0: one sample over the whole accumulated interval.
    let capacity = minted.estimates[0]
        .estimated_capacity_credits
        .expect("capacity");
    assert!((capacity - 1_500.0).abs() < 1e-6);
    let queries = repository.queries();
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0].0.sequence, 0);
    assert_eq!(queries[1].0.sequence, 0);
    assert_eq!(queries[1].1.sequence, 4);
}

/// Scenario E: a telemetry gap mid-accumulation invalidates everything since
/// the anchor. The estimator must not bridge `10% → 16%` across the gap.
#[tokio::test]
async fn storage_failure_during_accumulation_reanchors_instead_of_bridging() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let mut state = observe(&estimator, None, 10.0, None, checkpoint(), 0)
        .await
        .state;

    repository.push(priced(30.0));
    let accumulating = observe(&estimator, Some(state), 12.0, None, checkpoint(), 1).await;
    assert_eq!(
        accumulating.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    state = accumulating.state;

    let broken = observe(
        &estimator,
        Some(state),
        14.0,
        None,
        storage_failed_checkpoint(1),
        2,
    )
    .await;
    assert_eq!(
        broken.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    assert_eq!(
        broken.estimates[0]
            .latest_interval
            .storage_failed_request_logs,
        1
    );
    assert!(broken.estimates[0].estimated_capacity_credits.is_none());
    state = broken.state;

    // 14% → 16% is only 2 points: below the bootstrap threshold, so a bridge
    // across the gap would be the only way to mint here — and must not happen.
    repository.push(priced(30.0));
    let after_gap = observe(
        &estimator,
        Some(state),
        16.0,
        None,
        storage_failed_checkpoint(1),
        3,
    )
    .await;
    assert_eq!(
        after_gap.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    assert!(after_gap.estimates[0].estimated_capacity_credits.is_none());
    state = after_gap.state;

    // The recovered interval measures 14% → 20% with only post-gap cost.
    repository.push(priced(90.0));
    let recovered = observe(
        &estimator,
        Some(state),
        20.0,
        None,
        storage_failed_checkpoint(1),
        4,
    )
    .await;
    assert_eq!(
        recovered.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(
        recovered.estimates[0].estimated_capacity_credits,
        Some(1_500.0)
    );
    let queries = repository.queries();
    assert_eq!(queries.last().unwrap().0.sequence, 2);
}

/// A queue drop for this account during accumulation is a coverage gap: the
/// open interval is discarded, and confidence reports the loss.
#[tokio::test]
async fn queue_drop_invalidates_the_open_interval() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(Arc::clone(&repository) as _);
    let state = learned_capacity_state(&estimator, &repository, 10.0, None).await;

    let dropped = observe(
        &estimator,
        Some(state),
        26.0,
        None,
        queue_dropped_checkpoint(1),
        2,
    )
    .await;
    let estimate = &dropped.estimates[0];
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    assert_eq!(estimate.latest_interval.queue_dropped_request_logs, 1);
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Degraded);
    // The capacity prior itself is untouched.
    assert_eq!(estimate.estimated_capacity_credits, Some(1_500.0));
}
