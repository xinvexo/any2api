use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, HttpAccessLog, HttpAccessLogExchange,
    HttpAccessLogOutcome, HttpAccessLogSummary, HttpBodyCapture, HttpProtocolVersion, LogBatch,
    LogCursor, OAuthAccountId, ProtocolDialect, ProtocolOperation, RequestId, RequestLog,
    SettingKey, SettingOverrides, SettingValue, SettingsConfiguration,
};
use any2api_storage::api::{
    GatewayApiKeyLastUsedUpdate, GatewayApiKeyUsageRepository, GatewayApiKeyUsageSummary,
    HttpAccessLogCapacity, HttpAccessLogRepository, RequestLogCleanupOutcome, RequestLogRepository,
    StorageError, UpstreamCredentialUsageRepository, UpstreamCredentialUsageSummary,
};
use async_trait::async_trait;
use tokio::sync::Notify;

#[derive(Default)]
pub(super) struct BlockingRepository {
    pub(super) write_batches: AtomicUsize,
    pub(super) prune_calls: AtomicUsize,
    pub(super) access_prune_calls: AtomicUsize,
    pub(super) request_prune_deletions: AtomicUsize,
    pub(super) request_append_max_rows: AtomicU64,
    pub(super) request_prune_max_rows: AtomicU64,
    pub(super) request_append_has_more: AtomicBool,
    pub(super) request_prune_has_more: AtomicBool,
    pub(super) access_prune_deletions: AtomicUsize,
    pub(super) access_append_deletions: AtomicUsize,
    pub(super) fail_request_writes: AtomicBool,
    pub(super) release_first: Notify,
    pub(super) usage_updates: Mutex<Vec<Vec<GatewayApiKeyLastUsedUpdate>>>,
    pub(super) access_logs: Mutex<Vec<HttpAccessLog>>,
    pub(super) request_logs: Mutex<Vec<CompletedRequestLog>>,
}

#[async_trait]
impl HttpAccessLogRepository for BlockingRepository {
    async fn append_http_access_logs(
        &self,
        records: Vec<HttpAccessLog>,
        _capacity: HttpAccessLogCapacity,
    ) -> Result<u64, StorageError> {
        self.access_logs
            .lock()
            .expect("HTTP access logs")
            .extend(records);
        Ok(self.access_append_deletions.swap(0, Ordering::AcqRel) as u64)
    }

    async fn prune_http_access_logs(
        &self,
        _retention_before_ms: u64,
        _capacity: HttpAccessLogCapacity,
        _batch_size: u32,
    ) -> Result<u64, StorageError> {
        self.access_prune_calls.fetch_add(1, Ordering::AcqRel);
        Ok(self.access_prune_deletions.swap(0, Ordering::AcqRel) as u64)
    }

    async fn reclaim_http_access_log_storage(&self, _max_bytes: u64) -> Result<u64, StorageError> {
        Ok(0)
    }

    async fn list_http_access_logs(
        &self,
        _since_ms: u64,
        _show_admin_operations: bool,
        _cursor: Option<LogCursor>,
        limit: u32,
    ) -> Result<LogBatch<HttpAccessLogSummary>, StorageError> {
        let logs = self.access_logs.lock().expect("HTTP access logs");
        let items = logs
            .iter()
            .take(limit as usize)
            .map(HttpAccessLog::summary)
            .collect();
        Ok(LogBatch::new(items, None))
    }

    async fn get_http_access_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<HttpAccessLog>, StorageError> {
        Ok(self
            .access_logs
            .lock()
            .expect("HTTP access logs")
            .iter()
            .find(|log| log.request_id == request_id)
            .map(duplicate_access_log))
    }

    async fn clear_http_access_logs(&self) -> Result<u64, StorageError> {
        let mut logs = self.access_logs.lock().expect("HTTP access logs");
        let count = logs.len() as u64;
        logs.clear();
        Ok(count)
    }
}

fn duplicate_access_log(log: &HttpAccessLog) -> HttpAccessLog {
    HttpAccessLog {
        request_id: log.request_id,
        started_at_ms: log.started_at_ms,
        config_revision: log.config_revision,
        client_ip: log.client_ip,
        method: log.method.clone(),
        path: log.path.clone(),
        uri: log.uri.clone(),
        http_version: log.http_version,
        status_code: log.status_code,
        duration_ms: log.duration_ms,
        response_bytes: log.response_bytes,
        outcome: log.outcome,
        gateway_auth_rejected: log.gateway_auth_rejected,
        exchange: log.exchange.as_ref().map(|exchange| HttpAccessLogExchange {
            request_headers: exchange.request_headers.clone(),
            request_body: duplicate_body_capture(&exchange.request_body),
            response_headers: exchange.response_headers.clone(),
            response_body: duplicate_body_capture(&exchange.response_body),
        }),
    }
}

fn duplicate_body_capture(capture: &HttpBodyCapture) -> HttpBodyCapture {
    HttpBodyCapture::from_vec(
        capture.content().to_vec(),
        capture.total_bytes(),
        capture.is_complete(),
        capture.is_truncated(),
    )
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
        records: &[CompletedRequestLog],
        max_rows: u64,
    ) -> Result<RequestLogCleanupOutcome, StorageError> {
        self.request_append_max_rows
            .store(max_rows, Ordering::Release);
        let batch = self.write_batches.fetch_add(1, Ordering::AcqRel);
        if self.fail_request_writes.load(Ordering::Acquire) {
            return Err(StorageError::CorruptTelemetry);
        }
        if batch == 0 {
            self.release_first.notified().await;
        }
        self.request_logs
            .lock()
            .expect("request logs")
            .extend_from_slice(records);
        Ok(RequestLogCleanupOutcome::new(
            0,
            self.request_append_has_more.swap(false, Ordering::AcqRel),
        ))
    }

    async fn prune_request_logs(
        &self,
        _retention_before_ms: u64,
        max_rows: u64,
        _batch_size: u32,
    ) -> Result<RequestLogCleanupOutcome, StorageError> {
        self.request_prune_max_rows
            .store(max_rows, Ordering::Release);
        self.prune_calls.fetch_add(1, Ordering::AcqRel);
        Ok(RequestLogCleanupOutcome::new(
            self.request_prune_deletions.swap(0, Ordering::AcqRel) as u64,
            self.request_prune_has_more.swap(false, Ordering::AcqRel),
        ))
    }

    async fn list_request_logs(
        &self,
        _since_ms: u64,
        _filter: &any2api_domain::RequestLogFilter,
        _cursor: Option<LogCursor>,
        _limit: u32,
    ) -> Result<LogBatch<RequestLog>, StorageError> {
        Ok(LogBatch::empty())
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

pub(super) fn logging_settings(queue_capacity: u64) -> SettingsConfiguration {
    logging_settings_with_request_max_rows(queue_capacity, None)
}

pub(super) fn logging_settings_with_queue_limits(
    queue_capacity: u64,
    queue_max_bytes: u64,
) -> SettingsConfiguration {
    let overrides = SettingOverrides::from_entries([
        (
            SettingKey::LogsTelemetryQueueCapacity,
            SettingValue::Integer(queue_capacity),
        ),
        (
            SettingKey::LogsTelemetryQueueMaxBytes,
            SettingValue::Integer(queue_max_bytes),
        ),
    ])
    .expect("logging overrides");
    SettingsConfiguration::from_overrides(overrides).expect("logging settings")
}

pub(super) fn logging_settings_with_request_max_rows(
    queue_capacity: u64,
    request_max_rows: Option<u64>,
) -> SettingsConfiguration {
    let mut entries = vec![(
        SettingKey::LogsTelemetryQueueCapacity,
        SettingValue::Integer(queue_capacity),
    )];
    if let Some(max_rows) = request_max_rows {
        entries.push((
            SettingKey::LogsRequestMaxRows,
            SettingValue::Integer(max_rows),
        ));
    }
    let overrides = SettingOverrides::from_entries(entries).expect("logging override");
    SettingsConfiguration::from_overrides(overrides).expect("logging settings")
}

pub(super) fn record(request_id: RequestId) -> CompletedRequestLog {
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
            cache_creation_tokens: None,
            quota_cost: None,
            requested_speed_tier: None,
            effective_speed_tier: None,
            is_stream: false,
        },
        attempts: Vec::new(),
        telemetry_position: None,
    }
}

pub(super) fn oauth_record(request_id: RequestId, id: OAuthAccountId) -> CompletedRequestLog {
    let mut record = record(request_id);
    record.request.oauth_account_id = Some(id);
    record
}

pub(super) fn access_log(path: &str) -> HttpAccessLog {
    HttpAccessLog {
        request_id: RequestId::new(),
        started_at_ms: 1,
        config_revision: ConfigRevision::INITIAL,
        client_ip: None,
        method: "GET".to_owned(),
        path: path.to_owned(),
        uri: path.to_owned(),
        http_version: HttpProtocolVersion::Http11,
        status_code: Some(200),
        duration_ms: 1,
        response_bytes: 0,
        outcome: HttpAccessLogOutcome::Completed,
        gateway_auth_rejected: false,
        exchange: None,
    }
}

pub(super) async fn wait_for(condition: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        if condition() {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "condition was not reached"
        );
        tokio::task::yield_now().await;
    }
}
