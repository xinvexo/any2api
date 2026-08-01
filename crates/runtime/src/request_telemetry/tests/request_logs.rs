use std::sync::{Arc, atomic::Ordering};

use any2api_domain::{ConfigRevision, RequestId, SettingsConfiguration};

use super::support::{BlockingRepository, logging_settings, record, wait_for};
use crate::{lifecycle::ProcessLifecycle, request_telemetry::RequestTelemetry};

#[test]
fn disabled_request_telemetry_has_empty_metrics() {
    let telemetry = RequestTelemetry::disabled();

    assert_eq!(telemetry.metrics().queued_records, 0);
    assert_eq!(telemetry.metrics().dropped_records, 0);
    assert_eq!(telemetry.metrics().persisted_records, 0);
}

#[test]
fn disabled_request_telemetry_stays_disabled_for_published_settings() {
    let telemetry = RequestTelemetry::disabled();
    let settings = SettingsConfiguration::defaults();

    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());

    assert!(!policy.enabled);
    assert_eq!(telemetry.metrics().dropped_records, 0);
}

#[tokio::test]
async fn full_logical_queue_drops_without_waiting_for_the_writer() {
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

    telemetry.try_record(record(RequestId::new()), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    telemetry.try_record(record(RequestId::new()), policy);
    telemetry.try_record(record(RequestId::new()), policy);

    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 1);
    assert_eq!(metrics.dropped_records, 1);

    repository.release_first.notify_waiters();
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    assert_eq!(telemetry.metrics().persisted_records, 2);
}

#[tokio::test]
async fn request_log_change_epoch_advances_only_after_commit() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let mut changes = telemetry.subscribe_request_log_changes();
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());

    telemetry.try_record(record(RequestId::new()), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    assert_eq!(*changes.borrow(), 0);

    repository.release_first.notify_waiters();
    tokio::time::timeout(std::time::Duration::from_secs(1), changes.changed())
        .await
        .expect("request log commit notification")
        .expect("request log notifier remains open");
    assert_eq!(*changes.borrow_and_update(), 1);

    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn failed_request_log_batch_does_not_advance_change_epoch() {
    let repository = Arc::new(BlockingRepository::default());
    repository
        .fail_request_writes
        .store(true, Ordering::Release);
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let changes = telemetry.subscribe_request_log_changes();
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());

    telemetry.try_record(record(RequestId::new()), policy);
    wait_for(|| telemetry.metrics().dropped_records == 1).await;

    assert_eq!(*changes.borrow(), 0);
    assert!(
        !changes
            .has_changed()
            .expect("request log notifier remains open")
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test(start_paused = true)]
async fn writer_prunes_while_idle() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(1);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    ));

    wait_for(|| repository.prune_calls.load(Ordering::Acquire) >= 1).await;
    let initial_calls = repository.prune_calls.load(Ordering::Acquire);
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    wait_for(|| repository.prune_calls.load(Ordering::Acquire) > initial_calls).await;

    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn retention_deletions_advance_both_change_epochs() {
    let repository = Arc::new(BlockingRepository::default());
    repository
        .request_prune_deletions
        .store(2, Ordering::Release);
    repository
        .access_prune_deletions
        .store(3, Ordering::Release);
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let request_changes = telemetry.subscribe_request_log_changes();
    let access_changes = telemetry.subscribe_http_access_log_changes();

    wait_for(|| repository.prune_calls.load(Ordering::Acquire) >= 1).await;
    wait_for(|| repository.access_prune_calls.load(Ordering::Acquire) >= 1).await;
    assert_eq!(*request_changes.borrow(), 1);
    assert_eq!(*access_changes.borrow(), 1);

    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn shutdown_timeout_aborts_and_joins_the_writer() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(1);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    telemetry.try_record(record(RequestId::new()), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;

    telemetry
        .shutdown(std::time::Duration::from_millis(1))
        .await;
    lifecycle.close_background_tasks();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        lifecycle.wait_for_background_tasks(),
    )
    .await
    .expect("aborted writer must leave the task tracker");
    assert_eq!(lifecycle.background_task_count(), 0);
}
