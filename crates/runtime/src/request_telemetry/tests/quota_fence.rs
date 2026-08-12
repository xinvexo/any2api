use std::sync::{Arc, atomic::Ordering};

use any2api_domain::{ConfigRevision, OAuthAccountId, RequestId};

use super::support::{BlockingRepository, logging_settings, oauth_record, wait_for};
use crate::{lifecycle::ProcessLifecycle, request_telemetry::RequestTelemetry};

#[tokio::test]
async fn quota_observation_fences_completed_oauth_logs_by_monotonic_position() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    ));
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    let oauth_account_id = OAuthAccountId::new();

    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    let observation_task = {
        let telemetry = Arc::clone(&telemetry);
        tokio::spawn(async move { telemetry.quota_observation(oauth_account_id).await })
    };
    tokio::task::yield_now().await;
    assert!(!observation_task.is_finished());
    repository.release_first.notify_waiters();
    let first = observation_task.await.expect("quota observation task");
    assert!(first.checkpoint.enabled);
    assert_eq!(first.position.sequence, 1);
    let first_position = repository.request_logs.lock().unwrap()[0]
        .telemetry_position
        .expect("first telemetry position");
    assert_eq!(first_position, first.position);

    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    let second = telemetry.quota_observation(oauth_account_id).await;
    assert_eq!(second.position.process_id, first.position.process_id);
    assert_eq!(second.position.sequence, 2);
    {
        let logs = repository.request_logs.lock().unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(
            logs[1]
                .telemetry_position
                .expect("second telemetry position"),
            second.position
        );
    }

    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn loss_after_a_quota_boundary_is_not_absorbed_into_that_boundary() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(1);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    ));
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    let oauth_account_id = OAuthAccountId::new();
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;

    let later = {
        let telemetry = Arc::clone(&telemetry);
        let repository = Arc::clone(&repository);
        tokio::spawn(async move {
            telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
            repository.release_first.notify_waiters();
        })
    };
    let boundary = telemetry.quota_observation(oauth_account_id).await;
    later.await.expect("post-boundary request");
    assert_eq!(boundary.position.sequence, 1);
    assert_eq!(boundary.checkpoint.account_queue_dropped_request_logs, 0);

    let next = telemetry.quota_observation(oauth_account_id).await;
    assert_eq!(next.position.sequence, 2);
    assert_eq!(next.checkpoint.account_queue_dropped_request_logs, 1);
    assert!(
        !boundary
            .checkpoint
            .covers_interval_to(&next.checkpoint, boundary.position.sequence)
    );

    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn oauth_request_started_while_logging_was_disabled_marks_the_next_interval_incomplete() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let oauth_account_id = OAuthAccountId::new();
    let baseline = telemetry.quota_observation(oauth_account_id).await;
    let mut request_policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    request_policy.enabled = false;

    telemetry.try_record(
        oauth_record(RequestId::new(), oauth_account_id),
        request_policy,
    );
    let next = telemetry.quota_observation(oauth_account_id).await;

    assert_eq!(next.position.sequence, 1);
    assert_eq!(next.checkpoint.account_queue_dropped_request_logs, 1);
    assert!(
        !baseline
            .checkpoint
            .covers_interval_to(&next.checkpoint, baseline.position.sequence)
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn one_accounts_telemetry_loss_does_not_invalidate_another_accounts_interval() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let account_a = OAuthAccountId::new();
    let account_b = OAuthAccountId::new();
    let baseline_a = telemetry.quota_observation(account_a).await;
    let baseline_b = telemetry.quota_observation(account_b).await;
    let mut disabled_policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    disabled_policy.enabled = false;

    telemetry.try_record(oauth_record(RequestId::new(), account_b), disabled_policy);

    let next_a = telemetry.quota_observation(account_a).await;
    let next_b = telemetry.quota_observation(account_b).await;
    assert_eq!(next_a.checkpoint.account_queue_dropped_request_logs, 0);
    assert_eq!(next_b.checkpoint.account_queue_dropped_request_logs, 1);
    assert!(
        baseline_a
            .checkpoint
            .covers_interval_to(&next_a.checkpoint, baseline_a.position.sequence)
    );
    assert!(
        !baseline_b
            .checkpoint
            .covers_interval_to(&next_b.checkpoint, baseline_b.position.sequence)
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn prune_invalidates_only_intervals_it_reaches_into() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    let oauth_account_id = OAuthAccountId::new();

    repository.release_first.notify_waiters();
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) >= 1).await;
    repository.release_first.notify_waiters();
    let anchor = telemetry.quota_observation(oauth_account_id).await;
    assert_eq!(anchor.position.sequence, 2);

    // Retention deletes the first log: entirely before the anchor, harmless.
    prune_with_max_sequence(&telemetry, &repository, 1, 2).await;
    let after_history_prune = telemetry.quota_observation(oauth_account_id).await;
    assert_eq!(after_history_prune.checkpoint.pruned_through_sequence, 1);
    assert!(
        anchor
            .checkpoint
            .covers_interval_to(&after_history_prune.checkpoint, anchor.position.sequence)
    );

    // A later prune removes a log inside the open interval: fail closed.
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    prune_with_max_sequence(&telemetry, &repository, 3, 3).await;
    let after_interval_prune = telemetry.quota_observation(oauth_account_id).await;
    assert_eq!(after_interval_prune.checkpoint.pruned_through_sequence, 3);
    assert!(
        !anchor
            .checkpoint
            .covers_interval_to(&after_interval_prune.checkpoint, anchor.position.sequence)
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

async fn prune_with_max_sequence(
    telemetry: &RequestTelemetry,
    repository: &Arc<BlockingRepository>,
    max_deleted_sequence: u64,
    revision: u64,
) {
    let initial_prunes = repository.prune_calls.load(Ordering::Acquire);
    repository
        .prune_positions
        .lock()
        .expect("prune positions")
        .push(any2api_domain::RequestTelemetryPosition {
            process_id: telemetry.process_id_for_test(),
            sequence: max_deleted_sequence,
        });
    repository
        .request_prune_deletions
        .store(1, Ordering::Release);
    // Shrinking the retention window wakes the prune loop immediately.
    let lowered = super::support::logging_settings_with_retention(8, 60 * revision);
    telemetry.reconcile_policy_for_test(
        ConfigRevision::new(revision).expect("next revision"),
        lowered.logging(),
    );
    wait_for(|| repository.prune_calls.load(Ordering::Acquire) > initial_prunes).await;
}
