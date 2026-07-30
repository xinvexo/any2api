use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, GatewayApiKeyId, HttpAccessLog, HttpAccessLogOutcome,
    HttpProtocolVersion, LogPage, ProtocolDialect, ProtocolOperation, RequestId, RequestLog,
    SettingKey, SettingOverrides, SettingValue, SettingsConfiguration,
};
use any2api_storage::api::{
    GatewayApiKeyLastUsedUpdate, GatewayApiKeyUsageRepository, GatewayApiKeyUsageSummary,
    HttpAccessLogRepository, RequestLogRepository, StorageError, UpstreamCredentialUsageRepository,
    UpstreamCredentialUsageSummary,
};
use async_trait::async_trait;
use tokio::sync::Notify;

use crate::lifecycle::ProcessLifecycle;

use super::RequestTelemetry;

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
}

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

    telemetry.try_record_http_access(access_log("/before-clear"), settings.logging());
    assert_eq!(
        telemetry
            .clear_http_access_logs()
            .await
            .expect("clear HTTP access logs"),
        1
    );
    assert_eq!(*access_changes.borrow(), 2);
    telemetry.try_record_http_access(access_log("/after-clear"), settings.logging());
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

#[derive(Default)]
struct BlockingRepository {
    write_batches: AtomicUsize,
    prune_calls: AtomicUsize,
    access_prune_calls: AtomicUsize,
    request_prune_deletions: AtomicUsize,
    access_prune_deletions: AtomicUsize,
    fail_request_writes: AtomicBool,
    release_first: Notify,
    usage_updates: Mutex<Vec<Vec<GatewayApiKeyLastUsedUpdate>>>,
    access_logs: Mutex<Vec<HttpAccessLog>>,
}

#[async_trait]
impl HttpAccessLogRepository for BlockingRepository {
    async fn append_http_access_logs(&self, records: &[HttpAccessLog]) -> Result<(), StorageError> {
        self.access_logs
            .lock()
            .expect("HTTP access logs")
            .extend_from_slice(records);
        Ok(())
    }

    async fn prune_http_access_logs(
        &self,
        _retention_before_ms: u64,
        _max_rows: u64,
        _batch_size: u32,
    ) -> Result<u64, StorageError> {
        self.access_prune_calls.fetch_add(1, Ordering::AcqRel);
        Ok(self.access_prune_deletions.swap(0, Ordering::AcqRel) as u64)
    }

    async fn list_http_access_logs(
        &self,
        _since_ms: u64,
        offset: u64,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLog>, StorageError> {
        let logs = self.access_logs.lock().expect("HTTP access logs");
        let total = logs.len() as u64;
        let items = logs
            .iter()
            .skip(offset as usize)
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(LogPage::new(items, total))
    }

    async fn clear_http_access_logs(&self) -> Result<u64, StorageError> {
        let mut logs = self.access_logs.lock().expect("HTTP access logs");
        let count = logs.len() as u64;
        logs.clear();
        Ok(count)
    }
}

#[async_trait]
impl GatewayApiKeyUsageRepository for BlockingRepository {
    async fn touch_gateway_api_key_last_used(
        &self,
        updates: &[GatewayApiKeyLastUsedUpdate],
    ) -> Result<(), StorageError> {
        self.usage_updates
            .lock()
            .expect("usage updates")
            .push(updates.to_vec());
        Ok(())
    }

    async fn list_gateway_api_key_usage(
        &self,
    ) -> Result<Vec<GatewayApiKeyUsageSummary>, StorageError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl UpstreamCredentialUsageRepository for BlockingRepository {
    async fn list_upstream_credential_usage(
        &self,
    ) -> Result<Vec<UpstreamCredentialUsageSummary>, StorageError> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl RequestLogRepository for BlockingRepository {
    async fn append_request_logs(
        &self,
        _records: &[CompletedRequestLog],
    ) -> Result<(), StorageError> {
        let batch = self.write_batches.fetch_add(1, Ordering::AcqRel);
        if self.fail_request_writes.load(Ordering::Acquire) {
            return Err(StorageError::CorruptTelemetry);
        }
        if batch == 0 {
            self.release_first.notified().await;
        }
        Ok(())
    }

    async fn prune_request_logs(
        &self,
        _retention_before_ms: u64,
        _max_rows: u64,
        _batch_size: u32,
    ) -> Result<u64, StorageError> {
        self.prune_calls.fetch_add(1, Ordering::AcqRel);
        Ok(self.request_prune_deletions.swap(0, Ordering::AcqRel) as u64)
    }

    async fn list_request_logs(
        &self,
        _since_ms: u64,
        _offset: u64,
        _limit: u32,
    ) -> Result<LogPage<RequestLog>, StorageError> {
        Ok(LogPage::empty())
    }

    async fn get_request_log(
        &self,
        _request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError> {
        Ok(None)
    }

    async fn request_log_overview(
        &self,
        range: any2api_storage::api::RequestLogOverviewRange,
    ) -> Result<any2api_storage::api::RequestLogOverview, StorageError> {
        Ok(any2api_storage::api::RequestLogOverview::empty(range, 1))
    }
}

fn logging_settings(queue_capacity: u64) -> SettingsConfiguration {
    let overrides = SettingOverrides::from_entries([(
        SettingKey::LogsTelemetryQueueCapacity,
        SettingValue::Integer(queue_capacity),
    )])
    .expect("logging override");
    SettingsConfiguration::from_overrides(overrides).expect("logging settings")
}

fn record(request_id: RequestId) -> CompletedRequestLog {
    CompletedRequestLog {
        request: RequestLog {
            request_id,
            started_at_ms: 1,
            client_ip: "127.0.0.1".parse().expect("loopback address"),
            config_revision: ConfigRevision::INITIAL,
            gateway_api_key_id: None,
            ingress_protocol: ProtocolDialect::OpenAiResponses,
            operation: ProtocolOperation::Responses,
            public_model: Some("test".into()),
            thinking_level: None,
            provider_endpoint_id: None,
            credential_id: None,
            oauth_account_id: None,
            proxy_profile_id: None,
            status_code: 200,
            error_class: None,
            error_message: None,
            attempt_count: 0,
            latency_ms: 1,
            first_token_ms: None,
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            is_stream: false,
        },
        attempts: Vec::new(),
    }
}

fn access_log(path: &str) -> HttpAccessLog {
    HttpAccessLog {
        request_id: RequestId::new(),
        started_at_ms: 1,
        config_revision: ConfigRevision::INITIAL,
        client_ip: None,
        method: "GET".to_owned(),
        path: path.to_owned(),
        http_version: HttpProtocolVersion::Http11,
        status_code: Some(200),
        duration_ms: 1,
        response_bytes: 0,
        outcome: HttpAccessLogOutcome::Completed,
    }
}

async fn wait_for(condition: impl Fn() -> bool) {
    for _ in 0..10_000 {
        if condition() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("condition was not reached");
}
