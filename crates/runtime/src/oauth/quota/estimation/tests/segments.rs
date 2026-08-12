use std::sync::Arc;

use crate::oauth::quota::types::OAuthQuotaIntervalStatus;

use super::*;

#[tokio::test]
async fn sub_threshold_percentage_changes_accumulate_from_the_original_anchor() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(7.0));
    repository.push(priced(16.0));
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let first = observe(&estimator, None, 10.0, Some(100), checkpoint(0), 0).await;
    let second = observe(
        &estimator,
        Some(first.state),
        10.2,
        Some(100),
        checkpoint(0),
        1,
    )
    .await;
    assert_eq!(second.state.windows[0].sample_anchor.used_percent, 10.0);
    assert_eq!(second.state.windows[0].last_observation.used_percent, 10.2);
    let third = observe(
        &estimator,
        Some(second.state),
        10.4,
        Some(100),
        checkpoint(0),
        2,
    )
    .await;
    let accumulating = observe(
        &estimator,
        Some(third.state),
        10.7,
        Some(100),
        checkpoint(0),
        3,
    )
    .await;
    assert_eq!(accumulating.estimates[0].sample_count, 0);
    assert_eq!(
        accumulating.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    assert_eq!(
        accumulating.state.windows[0].sample_anchor.used_percent,
        10.0
    );

    let learned = observe(
        &estimator,
        Some(accumulating.state),
        11.6,
        Some(100),
        checkpoint(0),
        4,
    )
    .await;
    assert_eq!(learned.estimates[0].sample_count, 1);
    assert_eq!(
        learned.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert!(
        (learned.estimates[0]
            .latest_interval
            .delta_used_percent
            .expect("percentage delta")
            - 1.6)
            .abs()
            < 0.000_001
    );
    assert!((learned.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    let queries = repository.queries();
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0].0.sequence, 0);
    assert_eq!(queries[0].1.sequence, 3);
    assert_eq!(queries[1].0.sequence, 0);
    assert_eq!(queries[1].1.sequence, 4);
}

#[tokio::test]
async fn telemetry_gap_during_small_delta_accumulation_reanchors_before_learning() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(6.0));
    repository.push(priced(16.0));
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let first = observe(&estimator, None, 10.0, Some(100), checkpoint(0), 0).await;
    let partial = observe(
        &estimator,
        Some(first.state),
        10.2,
        Some(100),
        checkpoint(0),
        1,
    )
    .await;
    let gap = observe(
        &estimator,
        Some(partial.state),
        10.4,
        Some(100),
        checkpoint(1),
        2,
    )
    .await;
    assert_eq!(
        gap.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    assert_eq!(gap.estimates[0].sample_count, 0);
    assert_eq!(gap.state.windows[0].sample_anchor.used_percent, 10.4);
    let accumulating = observe(
        &estimator,
        Some(gap.state),
        11.0,
        Some(100),
        checkpoint(1),
        3,
    )
    .await;
    assert_eq!(
        accumulating.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    let learned = observe(
        &estimator,
        Some(accumulating.state),
        12.0,
        Some(100),
        checkpoint(1),
        4,
    )
    .await;
    assert_eq!(learned.estimates[0].sample_count, 1);
    assert!((learned.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    let queries = repository.queries();
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0].0.sequence, 2);
    assert_eq!(queries[0].1.sequence, 3);
    assert_eq!(queries[1].0.sequence, 2);
    assert_eq!(queries[1].1.sequence, 4);
}

#[tokio::test]
async fn reset_during_small_delta_accumulation_discards_the_old_anchor() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(6.0));
    repository.push(priced(16.0));
    let estimator = OAuthQuotaEstimator::new(repository.clone());
    let first = observe(&estimator, None, 10.0, Some(100), checkpoint(0), 0).await;
    let partial = observe(
        &estimator,
        Some(first.state),
        10.2,
        Some(100),
        checkpoint(0),
        1,
    )
    .await;
    let old_epoch = partial.estimates[0].epoch;
    let reset = observe(
        &estimator,
        Some(partial.state),
        0.0,
        Some(200),
        checkpoint(0),
        2,
    )
    .await;
    assert!(reset.estimates[0].epoch > old_epoch);
    assert_eq!(reset.estimates[0].sample_count, 0);
    assert_eq!(reset.state.windows[0].sample_anchor.used_percent, 0.0);
    let accumulating = observe(
        &estimator,
        Some(reset.state),
        0.6,
        Some(200),
        checkpoint(0),
        3,
    )
    .await;
    assert_eq!(
        accumulating.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    let learned = observe(
        &estimator,
        Some(accumulating.state),
        1.6,
        Some(200),
        checkpoint(0),
        4,
    )
    .await;
    assert_eq!(learned.estimates[0].sample_count, 1);
    assert!((learned.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    let queries = repository.queries();
    assert_eq!(queries.len(), 2);
    assert_eq!(queries[0].0.sequence, 2);
    assert_eq!(queries[0].1.sequence, 3);
    assert_eq!(queries[1].0.sequence, 2);
    assert_eq!(queries[1].1.sequence, 4);
}

#[tokio::test]
async fn contaminated_interval_salvages_the_verified_clean_prefix() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(20.0));
    repository.push(priced(20.0));
    repository.push(unpriced());
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
    let contaminated = observe(
        &estimator,
        Some(accumulating.state),
        4.6,
        Some(100),
        checkpoint(0),
        3,
    )
    .await;
    assert_eq!(
        contaminated.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::UnpricedUsage
    );
    assert_eq!(contaminated.estimates[0].sample_count, 2);
    assert!(
        (contaminated.estimates[0]
            .estimated_capacity_credits
            .unwrap()
            - 1_000.0)
            .abs()
            < 0.001
    );
    assert_eq!(
        contaminated.state.windows[0].sample_anchor.used_percent,
        4.6
    );
    assert!(contaminated.state.windows[0].segment.is_none());
}

#[tokio::test]
async fn restart_during_accumulation_salvages_the_verified_prefix() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(20.0));
    repository.push(priced(35.0));
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
        5.5,
        Some(100),
        checkpoint(0),
        2,
    )
    .await;
    assert_eq!(
        accumulating.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Accumulating
    );
    let restarted = observe(
        &estimator,
        Some(accumulating.state),
        6.0,
        Some(100),
        checkpoint_for(Uuid::new_v4(), 0),
        3,
    )
    .await;
    assert_eq!(
        restarted.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    assert_eq!(restarted.estimates[0].sample_count, 2);
    assert!((restarted.estimates[0].estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    assert!(restarted.state.windows[0].segment.is_none());
    assert!(restarted.state.valid());
}
