use std::sync::{Arc, RwLock};

use any2api_domain::{ConfigRevision, GatewayApiKeyId};
use any2api_storage::api::{
    GatewayApiKeyUsageRepository, HttpAccessLogRepository, RequestLogRepository,
};
use tokio::sync::{Notify, mpsc};

use super::support::{BlockingRepository, logging_settings, wait_for};
use crate::{
    lifecycle::ProcessLifecycle,
    request_telemetry::{
        RequestTelemetry,
        changes::LogChangeNotifier,
        event::TelemetryEvent,
        metrics::TelemetryCounters,
        policy::RequestLogPolicy,
        worker::{self, WorkerState},
    },
};

#[tokio::test]
async fn gateway_key_usage_is_live_immediately_and_duplicate_writes_are_throttled() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let lifecycle = ProcessLifecycle::new();
    let telemetry = RequestTelemetry::start(
        Arc::clone(&repository),
        ConfigRevision::INITIAL,
        settings.logging(),
        &lifecycle,
    );
    let id = GatewayApiKeyId::new();

    telemetry.record_gateway_key_use(id);
    let first = telemetry
        .gateway_key_last_used_at(id)
        .expect("live usage timestamp");
    telemetry.record_gateway_key_use(id);
    wait_for(|| {
        repository
            .usage_updates
            .lock()
            .expect("usage updates")
            .len()
            == 1
    })
    .await;

    let latest = telemetry
        .gateway_key_last_used_at(id)
        .expect("latest live usage timestamp");
    assert!(latest >= first);
    assert_eq!(
        repository
            .usage_updates
            .lock()
            .expect("usage updates")
            .len(),
        1
    );
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
    assert_eq!(telemetry.metrics().persisted_records, 1);
}

#[tokio::test]
async fn collapsed_gateway_updates_preserve_record_metrics() {
    let repository = Arc::new(BlockingRepository::default());
    let settings = logging_settings(8);
    let counters = TelemetryCounters::default();
    let policy = Arc::new(RwLock::new(RequestLogPolicy::from_settings(
        ConfigRevision::INITIAL,
        settings.logging(),
    )));
    let state = WorkerState {
        counters: counters.clone(),
        policy,
        changes: LogChangeNotifier::new(),
        prune_wakeup: Arc::new(Notify::new()),
        request_prune_wakeup: Arc::new(Notify::new()),
    };
    let (sender, receiver) = mpsc::channel(8);
    let id = GatewayApiKeyId::new();

    for last_used_at in ["2026-08-03 10:00:00", "2026-08-03 10:01:00"] {
        let event = TelemetryEvent::GatewayKeyLastUsed {
            id,
            last_used_at: last_used_at.to_owned(),
        };
        assert!(counters.try_reserve_slot(8));
        counters.enqueued(event.record_count());
        sender.try_send(event).expect("gateway usage event");
    }
    drop(sender);

    let request_logs: Arc<dyn RequestLogRepository> = Arc::clone(&repository) as _;
    let http_access_logs: Arc<dyn HttpAccessLogRepository> = Arc::clone(&repository) as _;
    let gateway_usage: Arc<dyn GatewayApiKeyUsageRepository> = Arc::clone(&repository) as _;
    worker::run(
        receiver,
        request_logs,
        http_access_logs,
        gateway_usage,
        state,
    )
    .await;

    let updates = repository.usage_updates.lock().expect("usage updates");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].len(), 1);
    assert_eq!(updates[0][0].last_used_at, "2026-08-03 10:01:00");
    let metrics = counters.snapshot();
    assert_eq!(metrics.queued_records, 0);
    assert_eq!(metrics.in_flight_records, 0);
    assert_eq!(metrics.dropped_records, 0);
    assert_eq!(metrics.persisted_records, 2);
}
