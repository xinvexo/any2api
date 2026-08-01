use std::sync::Arc;

use any2api_domain::ConfigRevision;

use super::support::{BlockingRepository, access_log, logging_settings, wait_for};
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
