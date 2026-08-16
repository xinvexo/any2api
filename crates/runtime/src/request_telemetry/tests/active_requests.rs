use std::sync::{Arc, atomic::Ordering};

use any2api_domain::{
    ConfigRevision, ErrorClass, GatewayApiKeyId, ProtocolOperation, RequestId, RequestLogFilter,
};

use super::support::{BlockingRepository, logging_settings, record, wait_for};
use crate::{
    lifecycle::ProcessLifecycle,
    request_telemetry::{RequestRecorder, RequestTelemetry},
};

#[tokio::test]
async fn active_request_remains_visible_until_the_final_log_commits() {
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
    let request_id = RequestId::new();
    let recorder = RequestRecorder::new(
        Arc::clone(&telemetry),
        policy,
        request_id,
        GatewayApiKeyId::new(),
        "203.0.113.8".parse().expect("client IP"),
        ProtocolOperation::Responses,
    );
    recorder.set_request_metadata("gpt-live".into(), true, Some("high".into()));

    let active = telemetry.list_active_requests(&RequestLogFilter::default(), 20);
    assert_eq!(active.total, 1);
    assert_eq!(active.items[0].request_id, request_id);
    assert_eq!(active.items[0].public_model.as_deref(), Some("gpt-live"));

    recorder.finish(200, None);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    assert_eq!(
        telemetry
            .list_active_requests(&RequestLogFilter::default(), 20)
            .total,
        1
    );

    repository.release_first.notify_waiters();
    wait_for(|| telemetry.metrics().persisted_records == 1).await;
    assert_eq!(
        telemetry
            .list_active_requests(&RequestLogFilter::default(), 20)
            .total,
        0
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn rejected_final_telemetry_removes_the_active_request() {
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

    let recorder = RequestRecorder::new(
        Arc::clone(&telemetry),
        policy,
        RequestId::new(),
        GatewayApiKeyId::new(),
        "203.0.113.8".parse().expect("client IP"),
        ProtocolOperation::Responses,
    );
    recorder.finish(200, None);

    assert_eq!(telemetry.metrics().dropped_records, 1);
    assert_eq!(
        telemetry
            .list_active_requests(&RequestLogFilter::default(), 20)
            .total,
        0
    );
    repository.release_first.notify_waiters();
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn storage_failure_removes_the_active_request() {
    let repository = Arc::new(BlockingRepository::default());
    repository
        .fail_request_writes
        .store(true, Ordering::Release);
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    ));
    let policy = telemetry.policy(ConfigRevision::INITIAL, settings.logging());
    let recorder = RequestRecorder::new(
        Arc::clone(&telemetry),
        policy,
        RequestId::new(),
        GatewayApiKeyId::new(),
        "203.0.113.8".parse().expect("client IP"),
        ProtocolOperation::Responses,
    );
    recorder.finish(502, Some(ErrorClass::Upstream));

    wait_for(|| telemetry.metrics().dropped_records == 1).await;
    assert_eq!(
        telemetry
            .list_active_requests(&RequestLogFilter::default(), 20)
            .total,
        0
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn dropping_a_request_recorder_persists_cancelled_and_clears_active() {
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
    let recorder = RequestRecorder::new(
        Arc::clone(&telemetry),
        policy,
        RequestId::new(),
        GatewayApiKeyId::new(),
        "203.0.113.8".parse().expect("client IP"),
        ProtocolOperation::Responses,
    );

    drop(recorder);
    wait_for(|| repository.write_batches.load(Ordering::Acquire) == 1).await;
    repository.release_first.notify_waiters();
    wait_for(|| telemetry.metrics().persisted_records == 1).await;

    {
        let records = repository.request_logs.lock().expect("request logs");
        assert_eq!(records[0].request.status_code, 499);
        assert_eq!(records[0].request.error_class, Some(ErrorClass::Cancelled));
    }
    assert_eq!(
        telemetry
            .list_active_requests(&RequestLogFilter::default(), 20)
            .total,
        0
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}
