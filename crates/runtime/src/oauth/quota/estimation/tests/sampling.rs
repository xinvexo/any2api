use std::sync::Arc;

use crate::oauth::quota::types::{OAuthQuotaEstimateConfidence, OAuthQuotaIntervalStatus};

use super::*;

#[tokio::test]
async fn stable_intervals_converge_with_median() {
    let repository = Arc::new(UsageRepository::default());
    for cost in [98.0, 100.0, 102.0] {
        repository.push(priced(cost));
    }
    let estimator = OAuthQuotaEstimator::new(repository);
    let mut state = None;
    for (index, percent) in [0.0, 10.0, 20.0, 30.0].into_iter().enumerate() {
        let result = observe(
            &estimator,
            state,
            percent,
            Some(99_999),
            checkpoint(0),
            index,
        )
        .await;
        state = Some(result.state);
        if index == 3 {
            let estimate = &result.estimates[0];
            assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Stable);
            assert_eq!(estimate.sample_count, 3);
            assert!((estimate.estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
        }
    }
}

#[tokio::test]
async fn cold_start_and_mid_window_start_wait_for_a_complete_interval() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(100.0));
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 60.0, Some(99_999), checkpoint(0), 0).await;
    assert_eq!(first.estimates[0].sample_count, 0);
    assert_eq!(
        first.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Unknown
    );
    assert_eq!(
        first.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::AwaitingBaseline
    );

    let second = observe(
        &estimator,
        Some(first.state),
        70.0,
        Some(99_999),
        checkpoint(0),
        1,
    )
    .await;
    assert_eq!(second.estimates[0].sample_count, 1);
    assert_eq!(
        second.estimates[0].confidence,
        OAuthQuotaEstimateConfidence::Learning
    );
}

#[tokio::test]
async fn telemetry_gaps_and_unpriced_usage_do_not_create_samples() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(unpriced());
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 0.0, Some(100), checkpoint(0), 0).await;
    let gap = observe(
        &estimator,
        Some(first.state),
        10.0,
        Some(100),
        checkpoint(1),
        1,
    )
    .await;
    assert_eq!(
        gap.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    let unpriced = observe(
        &estimator,
        Some(gap.state),
        20.0,
        Some(100),
        checkpoint(1),
        2,
    )
    .await;
    assert_eq!(unpriced.estimates[0].sample_count, 0);
    assert_eq!(
        unpriced.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::UnpricedUsage
    );
}

#[tokio::test]
async fn sqlite_write_failure_and_interval_query_failure_do_not_create_samples() {
    let repository = Arc::new(UsageRepository::default());
    repository.push_error();
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 0.0, Some(100), checkpoint(0), 0).await;
    let write_failed = observe(
        &estimator,
        Some(first.state),
        10.0,
        Some(100),
        storage_failed_checkpoint(1),
        1,
    )
    .await;
    assert_eq!(write_failed.estimates[0].sample_count, 0);
    assert_eq!(
        write_failed.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );
    assert_eq!(
        write_failed.estimates[0]
            .latest_interval
            .storage_failed_request_logs,
        1
    );

    let query_failed = observe(
        &estimator,
        Some(write_failed.state),
        20.0,
        Some(100),
        storage_failed_checkpoint(1),
        2,
    )
    .await;
    assert_eq!(query_failed.estimates[0].sample_count, 0);
    assert_eq!(
        query_failed.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::Invalid
    );
}

#[tokio::test]
async fn stable_distribution_rejects_external_and_outlier_intervals() {
    for (cost, expected) in [
        (10.0, OAuthQuotaIntervalStatus::ExternalUsageSuspected),
        (900.0, OAuthQuotaIntervalStatus::OutlierRejected),
    ] {
        let repository = Arc::new(UsageRepository::default());
        for value in [100.0, 100.0, 100.0, cost] {
            repository.push(priced(value));
        }
        let estimator = OAuthQuotaEstimator::new(repository);
        let mut state = None;
        let mut latest = None;
        for (index, percent) in [0.0, 10.0, 20.0, 30.0, 40.0].into_iter().enumerate() {
            let result = observe(&estimator, state, percent, Some(100), checkpoint(0), index).await;
            latest = Some(result.estimates[0].clone());
            state = Some(result.state);
        }
        let latest = latest.unwrap();
        assert_eq!(latest.sample_count, 3);
        assert_eq!(latest.latest_interval.status, expected);
        assert_eq!(latest.confidence, OAuthQuotaEstimateConfidence::Degraded);
        assert!((latest.estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
    }
}
