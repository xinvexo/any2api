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
        assert_eq!(state.unwrap().windows[0].competing_samples.len(), 1);
    }
}

#[tokio::test]
async fn stable_estimator_relearns_four_consistent_candidates_on_either_side() {
    for (replacement_cost, expected_capacity) in [(10.0, 100.0), (300.0, 3_000.0)] {
        let repository = Arc::new(UsageRepository::default());
        for cost in [100.0, 100.0, 100.0] {
            repository.push(priced(cost));
        }
        for _ in 0..4 {
            repository.push(priced(replacement_cost));
        }
        let estimator = OAuthQuotaEstimator::new(repository);
        let mut state = None;
        for (index, percent) in (0..=7).map(|value| value as f64 * 10.0).enumerate() {
            let result = observe(&estimator, state, percent, Some(100), checkpoint(0), index).await;
            if index == 6 {
                assert_eq!(result.estimates[0].sample_count, 3);
                assert_eq!(result.state.windows[0].competing_samples.len(), 3);
            }
            if index == 7 {
                assert_eq!(result.estimates[0].sample_count, 4);
                assert_eq!(
                    result.estimates[0].latest_interval.status,
                    OAuthQuotaIntervalStatus::ValidSample
                );
                assert_eq!(
                    result.estimates[0].confidence,
                    OAuthQuotaEstimateConfidence::Stable
                );
                assert!(
                    (result.estimates[0].estimated_capacity_credits.unwrap() - expected_capacity)
                        .abs()
                        < 0.001
                );
                assert!(result.state.windows[0].competing_samples.is_empty());
            }
            state = Some(result.state);
        }
    }
}

#[tokio::test]
async fn cold_start_low_cluster_can_relearn_the_consistent_true_capacity() {
    let repository = Arc::new(UsageRepository::default());
    for cost in [9.8, 10.0, 10.2, 98.0, 100.0, 102.0, 99.5] {
        repository.push(priced(cost));
    }
    let estimator = OAuthQuotaEstimator::new(repository);
    let mut state = None;
    let mut final_estimate = None;
    for (index, percent) in (0..=7).map(|value| value as f64 * 10.0).enumerate() {
        let result = observe(&estimator, state, percent, Some(100), checkpoint(0), index).await;
        final_estimate = Some(result.estimates[0].clone());
        state = Some(result.state);
    }
    let estimate = final_estimate.expect("final estimate");
    assert_eq!(estimate.sample_count, 4);
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Stable);
    assert_eq!(
        estimate.latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert!((estimate.estimated_capacity_credits.unwrap() - 997.5).abs() < 0.001);
}

#[tokio::test]
async fn one_high_outlier_is_cleared_by_normal_candidates_without_relearning() {
    let repository = Arc::new(UsageRepository::default());
    for cost in [100.0, 100.0, 100.0, 500.0, 99.0, 101.0] {
        repository.push(priced(cost));
    }
    let estimator = OAuthQuotaEstimator::new(repository);
    let mut state = None;
    let mut final_estimate = None;
    for (index, percent) in (0..=6).map(|value| value as f64 * 10.0).enumerate() {
        let result = observe(&estimator, state, percent, Some(100), checkpoint(0), index).await;
        if index == 4 {
            assert_eq!(
                result.estimates[0].latest_interval.status,
                OAuthQuotaIntervalStatus::OutlierRejected
            );
            assert_eq!(result.state.windows[0].competing_samples.len(), 1);
        }
        if index >= 5 {
            assert!(result.state.windows[0].competing_samples.is_empty());
        }
        final_estimate = Some(result.estimates[0].clone());
        state = Some(result.state);
    }
    let estimate = final_estimate.expect("final estimate");
    assert_eq!(estimate.sample_count, 5);
    assert_eq!(estimate.confidence, OAuthQuotaEstimateConfidence::Stable);
    assert!((estimate.estimated_capacity_credits.unwrap() - 1_000.0).abs() < 0.001);
}

#[tokio::test]
async fn wall_clock_rollback_does_not_change_sequence_interval_membership() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(100.0));
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(0.0, Some(100)),
            None,
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            telemetry_observation(checkpoint(0), 5_000, 0),
            None,
        )
        .await;
    let learned = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(10.0, Some(100)),
            Some(first.state),
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            telemetry_observation(checkpoint(0), 4_000, 1),
            None,
        )
        .await;

    assert_eq!(
        learned.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert_eq!(learned.estimates[0].sample_count, 1);
    assert!(learned.state.valid());
}
