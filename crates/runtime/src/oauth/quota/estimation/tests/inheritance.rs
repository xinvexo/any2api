use std::sync::Arc;

use crate::oauth::quota::types::{OAuthQuotaEstimateConfidence, OAuthQuotaIntervalStatus};

use super::*;

#[tokio::test]
async fn reset_rollover_inherits_samples_and_identity_changes_start_clean_epochs() {
    for current in [0.0, 3.0] {
        let repository = Arc::new(UsageRepository::default());
        let estimator = OAuthQuotaEstimator::new(repository);
        let first = observe(&estimator, None, 60.0, Some(100), checkpoint(0), 0).await;
        let learned = observe(
            &estimator,
            Some(first.state),
            70.0,
            Some(100),
            checkpoint(0),
            1,
        )
        .await;
        let epoch = learned.estimates[0].epoch;
        assert_eq!(learned.estimates[0].sample_count, 1);
        let reset = observe(
            &estimator,
            Some(learned.state),
            current,
            Some(100),
            checkpoint(0),
            2,
        )
        .await;
        assert!(reset.estimates[0].epoch > epoch);
        assert_eq!(reset.estimates[0].sample_count, 1);
        assert_eq!(reset.estimates[0].fresh_sample_count, 0);
        assert_eq!(
            reset.estimates[0].confidence,
            OAuthQuotaEstimateConfidence::Inherited
        );
        assert!(reset.estimates[0].estimated_capacity_credits.is_some());
        assert_eq!(
            reset.estimates[0].latest_interval.status,
            OAuthQuotaIntervalStatus::ResetBoundary
        );
    }

    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 0.0, Some(100), checkpoint(0), 0).await;
    let learned = observe(
        &estimator,
        Some(first.state),
        10.0,
        Some(100),
        checkpoint(0),
        1,
    )
    .await;
    let rollover = observe(
        &estimator,
        Some(learned.state),
        12.0,
        Some(200),
        checkpoint(0),
        2,
    )
    .await;
    assert_eq!(rollover.estimates[0].sample_count, 1);
    assert_eq!(rollover.estimates[0].fresh_sample_count, 0);
    let changed = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(20.0, Some(200)),
            Some(rollover.state),
            "identity-b".into(),
            QuotaCostUnit::CodexCredits,
            telemetry_observation(checkpoint(0), 4_000, 3),
            None,
        )
        .await;
    assert_eq!(changed.estimates[0].sample_count, 0);
    assert_eq!(
        changed.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Unknown
    );
    assert_eq!(
        changed.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );
}

#[tokio::test]
async fn rollover_salvages_accumulated_segment_and_fresh_sample_restores_stable() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(20.0));
    repository.push(priced(20.0));
    repository.push(priced(51.0));
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let baseline = observe(&estimator, None, 0.0, Some(100), checkpoint(0), 0).await;
    let minted = observe(
        &estimator,
        Some(baseline.state),
        2.0,
        Some(100),
        checkpoint(0),
        1,
    )
    .await;
    assert_eq!(minted.estimates[0].sample_count, 1);
    let accumulating = observe(
        &estimator,
        Some(minted.state),
        4.0,
        Some(100),
        checkpoint(0),
        2,
    )
    .await;
    assert_eq!(
        accumulating.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    let rollover = observe(
        &estimator,
        Some(accumulating.state),
        0.5,
        Some(200),
        checkpoint(0),
        3,
    )
    .await;
    assert_eq!(rollover.estimates[0].sample_count, 2);
    assert_eq!(rollover.estimates[0].fresh_sample_count, 0);
    assert_eq!(
        rollover.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Inherited
    );
    assert!((rollover.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);

    let confirmed = observe(
        &estimator,
        Some(rollover.state),
        5.6,
        Some(200),
        checkpoint(0),
        4,
    )
    .await;
    assert_eq!(confirmed.estimates[0].sample_count, 3);
    assert_eq!(confirmed.estimates[0].fresh_sample_count, 1);
    assert_eq!(
        confirmed.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Stable
    );
    assert!((confirmed.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
}

#[tokio::test]
async fn inherited_prior_is_not_lowered_by_contradicting_low_candidates() {
    let repository = Arc::new(UsageRepository::default());
    for cost in [100.0, 100.0, 100.0, 10.0, 10.0] {
        repository.push(priced(cost));
    }
    let estimator = OAuthQuotaEstimator::new(repository);
    let mut state = None;
    for (index, percent) in [0.0, 10.0, 20.0, 30.0].into_iter().enumerate() {
        let result = observe(&estimator, state, percent, Some(100), checkpoint(0), index).await;
        state = Some(result.state);
    }
    let rollover = observe(&estimator, state, 0.0, Some(200), checkpoint(0), 4).await;
    assert_eq!(rollover.estimates[0].sample_count, 3);
    assert_eq!(
        rollover.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Inherited
    );

    let rejected = observe(
        &estimator,
        Some(rollover.state),
        10.0,
        Some(200),
        checkpoint(0),
        5,
    )
    .await;
    assert_eq!(
        rejected.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ExternalUsageSuspected
    );
    assert_eq!(rejected.estimates[0].sample_count, 3);
    assert_eq!(rejected.state.windows[0].low_streak, 1);

    let still_high = observe(
        &estimator,
        Some(rejected.state),
        20.0,
        Some(200),
        checkpoint(0),
        6,
    )
    .await;
    assert_eq!(
        still_high.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ExternalUsageSuspected
    );
    assert_eq!(still_high.estimates[0].sample_count, 3);
    assert_eq!(still_high.estimates[0].fresh_sample_count, 0);
    assert_eq!(
        still_high.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Degraded
    );
    assert!((still_high.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    assert_eq!(still_high.state.windows[0].low_streak, 2);
    assert!(still_high.state.valid());
}

#[tokio::test]
async fn inherited_prior_is_corrected_upward_by_two_consistent_fresh_highs() {
    let repository = Arc::new(UsageRepository::default());
    for cost in [10.0, 10.0, 10.0, 100.0, 100.0] {
        repository.push(priced(cost));
    }
    let estimator = OAuthQuotaEstimator::new(repository);
    let mut state = None;
    for (index, percent) in [0.0, 10.0, 20.0, 30.0].into_iter().enumerate() {
        let result = observe(&estimator, state, percent, Some(100), checkpoint(0), index).await;
        state = Some(result.state);
    }
    let rollover = observe(&estimator, state, 0.0, Some(200), checkpoint(0), 4).await;
    assert_eq!(rollover.estimates[0].sample_count, 3);
    assert_eq!(
        rollover.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Inherited
    );
    assert!((rollover.estimates[0].estimated_capacity_credits.unwrap() - 100.0).abs() < 0.001);

    let pending = observe(
        &estimator,
        Some(rollover.state),
        10.0,
        Some(200),
        checkpoint(0),
        5,
    )
    .await;
    assert_eq!(
        pending.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::OutlierRejected
    );
    assert_eq!(pending.estimates[0].sample_count, 3);
    assert!((pending.estimates[0].estimated_capacity_credits.unwrap() - 100.0).abs() < 0.001);
    assert!(pending.state.windows[0].pending_high.is_some());

    let adopted = observe(
        &estimator,
        Some(pending.state),
        20.0,
        Some(200),
        checkpoint(0),
        6,
    )
    .await;
    assert_eq!(
        adopted.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(adopted.estimates[0].sample_count, 2);
    assert_eq!(adopted.estimates[0].fresh_sample_count, 2);
    assert_eq!(
        adopted.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Learning
    );
    assert!((adopted.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    assert!(adopted.state.windows[0].pending_high.is_none());
}
