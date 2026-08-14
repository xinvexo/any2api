use std::sync::{Arc, atomic::Ordering};

use any2api_domain::{ConfigRevision, OAuthAccountId, RequestId, SettingsConfiguration};
use any2api_storage::api::{RequestLogRepository, SqliteStore};
use tempfile::tempdir;

use super::support::{
    BlockingRepository, logging_settings, logging_settings_with_request_max_rows, oauth_record,
    record, wait_for,
};
use crate::{lifecycle::ProcessLifecycle, request_telemetry::RequestTelemetry};

#[test]
fn disabled_request_telemetry_has_empty_metrics() {
    let telemetry = RequestTelemetry::disabled();

    assert_eq!(telemetry.metrics().queued_records, 0);
    assert_eq!(telemetry.metrics().in_flight_records, 0);
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
async fn request_policy_lookup_is_pure_until_configuration_reconcile() {
    let repository = Arc::new(BlockingRepository::default());
    let initial = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        initial.logging(),
        &lifecycle,
    );
    let next_revision = ConfigRevision::new(2).expect("next revision");
    let next = logging_settings_with_request_max_rows(4, Some(10));

    let request_policy = telemetry.policy(next_revision, next.logging());

    assert_eq!(request_policy.revision, next_revision);
    assert_eq!(request_policy.queue_capacity, 4);
    assert_eq!(telemetry.current_policy().revision, ConfigRevision::INITIAL);
    assert_eq!(telemetry.current_policy().queue_capacity, 8);

    telemetry.reconcile_policy_for_test(next_revision, next.logging());
    assert_eq!(telemetry.current_policy(), request_policy);
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
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
    let oauth_account_id = OAuthAccountId::new();

    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);

    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 1);
    assert_eq!(metrics.in_flight_records, 1);
    assert_eq!(metrics.dropped_records, 1);

    repository.release_first.notify_waiters();
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 0);
    assert_eq!(metrics.in_flight_records, 0);
    assert_eq!(metrics.dropped_records, 1);
    assert_eq!(metrics.persisted_records, 2);
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

    let initial_prunes = repository.prune_calls.load(Ordering::Acquire);
    repository
        .request_append_has_more
        .store(true, Ordering::Release);
    repository.release_first.notify_waiters();
    tokio::time::timeout(std::time::Duration::from_secs(1), changes.changed())
        .await
        .expect("request log commit notification")
        .expect("request log notifier remains open");
    assert_eq!(*changes.borrow_and_update(), 1);
    wait_for(|| repository.prune_calls.load(Ordering::Acquire) > initial_prunes).await;
    assert_eq!(
        repository.request_append_max_rows.load(Ordering::Acquire),
        settings.logging().request_max_rows()
    );

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

    let oauth_account_id = OAuthAccountId::new();
    telemetry.try_record(oauth_record(RequestId::new(), oauth_account_id), policy);
    wait_for(|| telemetry.metrics().dropped_records == 1).await;

    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 0);
    assert_eq!(metrics.in_flight_records, 0);
    assert_eq!(metrics.persisted_records, 0);
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
async fn published_row_cap_wakes_cleanup_without_waiting_for_the_interval() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings_with_request_max_rows(8, Some(200));
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    wait_for(|| repository.prune_calls.load(Ordering::Acquire) >= 1).await;
    wait_for(|| repository.access_prune_calls.load(Ordering::Acquire) >= 1).await;
    let initial_calls = repository.prune_calls.load(Ordering::Acquire);
    let initial_access_calls = repository.access_prune_calls.load(Ordering::Acquire);
    repository
        .request_prune_has_more
        .store(true, Ordering::Release);
    repository
        .request_prune_deletions
        .store(1, Ordering::Release);
    let lowered = logging_settings_with_request_max_rows(8, Some(10));

    telemetry.reconcile_policy_for_test(
        ConfigRevision::new(2).expect("next revision"),
        lowered.logging(),
    );

    wait_for(|| repository.prune_calls.load(Ordering::Acquire) >= initial_calls + 2).await;
    assert_eq!(
        repository.request_prune_max_rows.load(Ordering::Acquire),
        10
    );
    assert_eq!(
        repository.access_prune_calls.load(Ordering::Acquire),
        initial_access_calls + 1
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn burst_writes_above_the_old_cleanup_rate_stay_within_the_row_cap() {
    let directory = tempdir().expect("temporary directory");
    let repository = Arc::new(
        SqliteStore::connect(&directory.path().join("request-log-burst.sqlite3"))
            .await
            .expect("storage"),
    );
    let settings = logging_settings_with_request_max_rows(2_048, Some(100));
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());

    for _ in 0..1_200 {
        telemetry.try_record(record(RequestId::new()), policy);
    }
    wait_for(|| telemetry.metrics().persisted_records == 1_200).await;

    let page = repository
        .list_request_logs(0, &Default::default(), None, 1, 200)
        .await
        .expect("capped request log page");
    assert_eq!(page.total, 100);
    assert_eq!(page.items.len(), 100);
    assert_eq!(telemetry.metrics().dropped_records, 0);
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
async fn graceful_shutdown_flushes_all_queued_request_logs() {
    let directory = tempdir().expect("temporary directory");
    let repository = Arc::new(
        SqliteStore::connect(&directory.path().join("request-log-shutdown.sqlite3"))
            .await
            .expect("storage"),
    );
    let settings = logging_settings_with_request_max_rows(1_024, Some(1_024));
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Unix time")
        .as_millis() as u64;
    for _ in 0..500 {
        let mut log = record(RequestId::new());
        log.request.started_at_ms = started_at_ms;
        telemetry.try_record(log, policy);
    }

    telemetry.shutdown(std::time::Duration::from_secs(10)).await;

    assert_eq!(telemetry.metrics().persisted_records, 500);
    assert_eq!(telemetry.metrics().dropped_records, 0);
    let page = repository
        .list_request_logs(0, &Default::default(), None, 1, 1)
        .await
        .expect("persisted request logs");
    assert_eq!(page.total, 500);
}

#[tokio::test]
async fn shutdown_timeout_aborts_and_joins_the_writer() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(4);
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
    telemetry.try_record(record(RequestId::new()), policy);
    telemetry.try_record(record(RequestId::new()), policy);

    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 2);
    assert_eq!(metrics.in_flight_records, 1);
    assert_eq!(metrics.dropped_records, 0);
    assert_eq!(metrics.persisted_records, 0);

    telemetry
        .shutdown(std::time::Duration::from_millis(1))
        .await;
    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 0);
    assert_eq!(metrics.in_flight_records, 0);
    assert_eq!(metrics.dropped_records, 3);
    assert_eq!(metrics.persisted_records, 0);
    lifecycle.close_background_tasks();
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        lifecycle.wait_for_background_tasks(),
    )
    .await
    .expect("aborted writer must leave the task tracker");
    assert_eq!(lifecycle.background_task_count(), 0);
}
