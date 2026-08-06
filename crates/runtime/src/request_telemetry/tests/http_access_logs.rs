use std::sync::Arc;
use std::sync::atomic::Ordering;

use any2api_domain::{
    ConfigRevision, HttpAccessLogExchange, HttpBodyCapture, MIN_TELEMETRY_QUEUE_MAX_BYTES,
    RequestId,
};

use super::support::{
    BlockingRepository, access_log, logging_settings, logging_settings_with_queue_limits, record,
    wait_for,
};
use crate::{
    lifecycle::ProcessLifecycle,
    request_telemetry::{HttpAccessLogChangeNotification, RequestTelemetry},
};

#[tokio::test]
async fn clear_http_access_logs_is_ordered_with_regular_events() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let access_changes = telemetry.subscribe_http_access_log_changes();

    telemetry.try_record_http_access(
        access_log("/before-clear"),
        settings.logging(),
        HttpAccessLogChangeNotification::Notify,
    );
    assert_eq!(
        telemetry
            .clear_http_access_logs()
            .await
            .expect("clear HTTP access logs"),
        1
    );
    assert_eq!(*access_changes.borrow(), 2);
    telemetry.try_record_http_access(
        access_log("/after-clear"),
        settings.logging(),
        HttpAccessLogChangeNotification::Notify,
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    assert_eq!(*access_changes.borrow(), 3);
    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 0);
    assert_eq!(metrics.in_flight_records, 0);
    assert_eq!(metrics.dropped_records, 0);
    assert_eq!(metrics.persisted_records, 2);

    assert_eq!(
        repository
            .access_logs
            .lock()
            .expect("HTTP access logs")
            .iter()
            .map(|log| log.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/after-clear"]
    );
}

#[tokio::test]
async fn suppressed_http_access_log_change_is_persisted_without_advancing_epoch() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let changes = telemetry.subscribe_http_access_log_changes();

    telemetry.try_record_http_access(
        access_log("/api/admin/system-logs"),
        settings.logging(),
        HttpAccessLogChangeNotification::Suppress,
    );
    wait_for(|| {
        repository
            .access_logs
            .lock()
            .expect("HTTP access logs")
            .len()
            == 1
    })
    .await;
    telemetry.try_record_http_access(
        access_log("/api/health"),
        settings.logging(),
        HttpAccessLogChangeNotification::Notify,
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    assert_eq!(*changes.borrow(), 1);
    assert_eq!(
        repository
            .access_logs
            .lock()
            .expect("HTTP access logs")
            .len(),
        2
    );
}

#[tokio::test]
async fn capacity_eviction_advances_epoch_for_a_suppressed_batch() {
    let repository = Arc::new(BlockingRepository::default());
    repository
        .access_append_deletions
        .store(1, Ordering::Release);
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let changes = telemetry.subscribe_http_access_log_changes();

    telemetry.try_record_http_access(
        access_log("/api/admin/system-logs"),
        settings.logging(),
        HttpAccessLogChangeNotification::Suppress,
    );
    wait_for(|| {
        repository
            .access_logs
            .lock()
            .expect("HTTP access logs")
            .len()
            == 1
    })
    .await;
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;

    assert_eq!(*changes.borrow(), 1);
}

#[tokio::test]
async fn gateway_auth_rejections_cannot_fill_the_shared_logical_queue() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(4);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let request_policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    telemetry.try_record(record(RequestId::new()), request_policy);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;

    for index in 0..2 {
        let mut rejected = access_log(&format!("/v1/rejected-{index}"));
        rejected.gateway_auth_rejected = true;
        telemetry.try_record_http_access(
            rejected,
            settings.logging(),
            HttpAccessLogChangeNotification::Notify,
        );
    }
    for index in 0..3 {
        telemetry.try_record_http_access(
            access_log(&format!("/protected-{index}")),
            settings.logging(),
            HttpAccessLogChangeNotification::Notify,
        );
    }

    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 4);
    assert_eq!(metrics.in_flight_records, 1);
    assert_eq!(metrics.dropped_records, 1);

    repository.release_first.notify_waiters();
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    let logs = repository.access_logs.lock().expect("HTTP access logs");
    assert_eq!(logs.len(), 4);
    assert_eq!(
        logs.iter().filter(|log| log.gateway_auth_rejected).count(),
        1
    );
    assert_eq!(telemetry.metrics().persisted_records, 5);
}

#[tokio::test]
async fn telemetry_owned_bytes_remain_bounded_while_storage_is_blocked() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings_with_queue_limits(8, MIN_TELEMETRY_QUEUE_MAX_BYTES);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    telemetry.try_record(
        record(RequestId::new()),
        telemetry.policy(ConfigRevision::INITIAL, settings.logging()),
    );
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;

    for path in ["/large-accepted", "/large-dropped"] {
        let mut log = access_log(path);
        log.exchange = Some(HttpAccessLogExchange {
            request_headers: Vec::new(),
            request_body: body_capture(1024 * 1024),
            response_headers: Vec::new(),
            response_body: body_capture(1024 * 1024),
        });
        telemetry.try_record_http_access(
            log,
            settings.logging(),
            HttpAccessLogChangeNotification::Notify,
        );
    }
    telemetry.try_record_http_access(
        access_log("/small-accepted"),
        settings.logging(),
        HttpAccessLogChangeNotification::Notify,
    );

    let metrics = telemetry.metrics();
    assert_eq!(metrics.queued_records, 2);
    assert_eq!(metrics.in_flight_records, 1);
    assert_eq!(metrics.dropped_records, 1);

    repository.release_first.notify_waiters();
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    let logs = repository.access_logs.lock().expect("HTTP access logs");
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].path, "/large-accepted");
    assert_eq!(logs[1].path, "/small-accepted");
    assert_eq!(telemetry.metrics().persisted_records, 3);
}

fn body_capture(bytes: usize) -> HttpBodyCapture {
    HttpBodyCapture::from_vec(
        vec![0; bytes],
        u64::try_from(bytes).expect("body size fits u64"),
        true,
        false,
    )
}
