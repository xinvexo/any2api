use std::sync::Arc;

use crate::oauth::quota::types::OAuthQuotaIntervalStatus;

use super::*;

#[tokio::test]
async fn reset_rollover_and_identity_changes_start_clean_epochs() {
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
        assert_eq!(reset.estimates[0].sample_count, 0);
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
    assert_eq!(rollover.estimates[0].sample_count, 0);
    let changed = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(20.0, Some(200)),
            Some(rollover.state),
            "identity-b".into(),
            QuotaCostUnit::CodexCredits,
            checkpoint(0),
            4_000,
            None,
        )
        .await;
    assert_eq!(changed.estimates[0].sample_count, 0);
    assert_eq!(
        changed.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );
}

#[tokio::test]
async fn restart_without_reset_identity_and_natural_rollover_start_new_epochs() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 0.0, None, checkpoint(0), 0).await;
    let learned = observe(&estimator, Some(first.state), 10.0, None, checkpoint(0), 1).await;
    let learned_epoch = learned.estimates[0].epoch;
    let restarted = observe(
        &estimator,
        Some(learned.state),
        20.0,
        None,
        checkpoint_for(Uuid::new_v4(), 0),
        2,
    )
    .await;
    assert!(restarted.estimates[0].epoch > learned_epoch);
    assert_eq!(restarted.estimates[0].sample_count, 0);
    assert_eq!(
        restarted.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );

    let first = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(0.0, None),
            None,
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            checkpoint(0),
            1_000,
            None,
        )
        .await;
    let learned = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(10.0, None),
            Some(first.state),
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            checkpoint(0),
            2_000,
            None,
        )
        .await;
    let learned_epoch = learned.estimates[0].epoch;
    let rolled_over = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(12.0, None),
            Some(learned.state),
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            checkpoint(0),
            18_001_000,
            None,
        )
        .await;
    assert!(rolled_over.estimates[0].epoch > learned_epoch);
    assert_eq!(rolled_over.estimates[0].sample_count, 0);
}

#[tokio::test]
async fn stable_reset_timestamp_is_stronger_than_elapsed_window_duration() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 0.0, Some(99_999), checkpoint(0), 0).await;
    let learned = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &usage(10.0, Some(99_999)),
            Some(first.state),
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            checkpoint(0),
            18_001_000,
            None,
        )
        .await;

    assert_eq!(learned.estimates[0].epoch, 1);
    assert_eq!(learned.estimates[0].sample_count, 1);
    assert_eq!(
        learned.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
}

#[tokio::test]
async fn one_second_reset_timestamp_drift_keeps_epoch_and_learns_interval() {
    let repository = Arc::new(UsageRepository::default());
    repository.push(priced(2.514804));
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 1.0, Some(1_789_041_126), checkpoint(0), 0).await;
    let epoch = first.estimates[0].epoch;
    let learned = observe(
        &estimator,
        Some(first.state),
        11.0,
        Some(1_789_041_127),
        checkpoint(0),
        1,
    )
    .await;

    assert_eq!(learned.estimates[0].epoch, epoch);
    assert_eq!(learned.estimates[0].sample_count, 1);
    assert_eq!(
        learned.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
    assert!(
        (learned.estimates[0]
            .estimated_capacity_credits
            .expect("capacity")
            - 25.14804)
            .abs()
            < 0.000_001
    );
    assert_eq!(
        learned.state.windows[0].baseline.reset_at,
        Some(1_789_041_127)
    );
}

#[tokio::test]
async fn changed_window_identity_starts_a_reset_boundary() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 0.0, Some(99_999), checkpoint(0), 0).await;
    let first_epoch = first.estimates[0].epoch;
    let mut changed_usage = usage(10.0, Some(99_999));
    changed_usage.rate_limit.as_mut().unwrap().windows[0].limit_window_seconds = Some(3_600);
    let changed = estimator
        .observe(
            OAuthAccountId::from_uuid(Uuid::nil()),
            &changed_usage,
            Some(first.state),
            "identity-a".into(),
            QuotaCostUnit::CodexCredits,
            checkpoint(0),
            2_000,
            None,
        )
        .await;

    assert!(changed.estimates[0].epoch > first_epoch);
    assert_eq!(changed.estimates[0].sample_count, 0);
    assert_eq!(
        changed.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ResetBoundary
    );
}

#[tokio::test]
async fn small_negative_jitter_does_not_reset_or_discard_samples() {
    let repository = Arc::new(UsageRepository::default());
    let estimator = OAuthQuotaEstimator::new(repository);
    let first = observe(&estimator, None, 40.0, Some(100), checkpoint(0), 0).await;
    let learned = observe(
        &estimator,
        Some(first.state),
        50.0,
        Some(100),
        checkpoint(0),
        1,
    )
    .await;
    let epoch = learned.estimates[0].epoch;
    let jitter = observe(
        &estimator,
        Some(learned.state),
        49.8,
        Some(100),
        checkpoint(0),
        2,
    )
    .await;
    assert_eq!(jitter.estimates[0].epoch, epoch);
    assert_eq!(jitter.estimates[0].sample_count, 1);
    assert_eq!(
        jitter.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::NoChange
    );
}

#[tokio::test]
async fn restart_gap_preserves_proven_epoch_and_next_clean_interval_recovers() {
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
    let epoch = learned.estimates[0].epoch;
    let persisted = serde_json::to_vec(&learned.state).expect("persist estimator state");
    let restored: QuotaEstimatorState =
        serde_json::from_slice(&persisted).expect("restore estimator state");
    assert!(restored.valid());
    let restarted_process = Uuid::new_v4();
    let restarted = observe(
        &estimator,
        Some(restored),
        20.0,
        Some(100),
        checkpoint_for(restarted_process, 0),
        2,
    )
    .await;
    assert_eq!(restarted.estimates[0].epoch, epoch);
    assert_eq!(restarted.estimates[0].sample_count, 1);
    assert_eq!(
        restarted.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::TelemetryIncomplete
    );

    let recovered = observe(
        &estimator,
        Some(restarted.state),
        30.0,
        Some(100),
        checkpoint_for(restarted_process, 0),
        3,
    )
    .await;
    assert_eq!(recovered.estimates[0].epoch, epoch);
    assert_eq!(recovered.estimates[0].sample_count, 2);
    assert_eq!(
        recovered.estimates[0].latest_interval.status,
        OAuthQuotaIntervalStatus::ValidSample
    );
}
