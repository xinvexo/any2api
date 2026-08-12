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
        tokio::spawn(async move { telemetry.quota_observation().await })
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
    let second = telemetry.quota_observation().await;
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
    let boundary = telemetry.quota_observation().await;
    later.await.expect("post-boundary request");
    assert_eq!(boundary.position.sequence, 1);
    assert_eq!(boundary.checkpoint.queue_dropped_request_logs, 0);

    let next = telemetry.quota_observation().await;
    assert_eq!(next.position.sequence, 2);
    assert_eq!(next.checkpoint.queue_dropped_request_logs, 1);
    assert!(!boundary.checkpoint.covers_interval_to(&next.checkpoint));

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
    let baseline = telemetry.quota_observation().await;
    let mut request_policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    request_policy.enabled = false;

    telemetry.try_record(
        oauth_record(RequestId::new(), OAuthAccountId::new()),
        request_policy,
    );
    let next = telemetry.quota_observation().await;

    assert_eq!(next.position.sequence, 1);
    assert_eq!(next.checkpoint.queue_dropped_request_logs, 1);
    assert!(!baseline.checkpoint.covers_interval_to(&next.checkpoint));
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}
