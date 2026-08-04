use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, GatewayApiKeyId, HttpAccessLog, HttpAccessLogSummary,
    LogPage, LogPageCursor, LoggingSettings, MAX_TELEMETRY_QUEUE_CAPACITY, RequestId, RequestLog,
};
use any2api_storage::api::{
    GatewayApiKeyUsageRepository, GatewayApiKeyUsageSummary, HttpAccessLogRepository,
    RequestLogOverview, RequestLogOverviewRange, RequestLogRepository, StorageError,
    UpstreamCredentialUsageRepository, UpstreamCredentialUsageSummary,
};
use tokio::{
    sync::{Notify, mpsc},
    task::JoinHandle,
};

use super::{
    changes::LogChangeNotifier,
    event::{HttpAccessLogChangeNotification, TelemetryEnvelope, TelemetryEvent},
    gateway_usage::{GatewayUsageTracker, utc_timestamp},
    metrics::{RequestTelemetryMetrics, TelemetryCounters},
    policy::RequestLogPolicy,
    worker,
};
use crate::{
    configuration::{PublishedSnapshot, PublishedSnapshotReconciler},
    lifecycle::ProcessLifecycle,
};

#[derive(Debug, thiserror::Error)]
pub enum RequestTelemetryControlError {
    #[error("request telemetry writer is unavailable")]
    WriterUnavailable,
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub struct RequestTelemetry {
    request_logs: Option<Arc<dyn RequestLogRepository>>,
    http_access_logs: Option<Arc<dyn HttpAccessLogRepository>>,
    gateway_usage_repository: Option<Arc<dyn GatewayApiKeyUsageRepository>>,
    upstream_usage_repository: Option<Arc<dyn UpstreamCredentialUsageRepository>>,
    sender: RwLock<Option<mpsc::Sender<TelemetryEnvelope>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    counters: TelemetryCounters,
    policy: Arc<RwLock<RequestLogPolicy>>,
    prune_wakeup: Arc<Notify>,
    gateway_usage: Mutex<GatewayUsageTracker>,
    changes: LogChangeNotifier,
}

impl RequestTelemetry {
    #[must_use]
    pub fn disabled() -> Self {
        let mut policy = RequestLogPolicy::from_settings(
            ConfigRevision::INITIAL,
            &any2api_domain::SettingsConfiguration::defaults()
                .logging()
                .clone(),
        );
        policy.enabled = false;
        Self {
            request_logs: None,
            http_access_logs: None,
            gateway_usage_repository: None,
            upstream_usage_repository: None,
            sender: RwLock::new(None),
            worker: Mutex::new(None),
            counters: TelemetryCounters::default(),
            policy: Arc::new(RwLock::new(policy)),
            prune_wakeup: Arc::new(Notify::new()),
            gateway_usage: Mutex::new(GatewayUsageTracker::default()),
            changes: LogChangeNotifier::new(),
        }
    }

    pub fn start<R>(
        repository: Arc<R>,
        revision: ConfigRevision,
        settings: &LoggingSettings,
        lifecycle: &ProcessLifecycle,
    ) -> Self
    where
        R: RequestLogRepository
            + HttpAccessLogRepository
            + GatewayApiKeyUsageRepository
            + UpstreamCredentialUsageRepository
            + 'static,
    {
        let request_logs: Arc<dyn RequestLogRepository> = Arc::clone(&repository) as _;
        let http_access_logs: Arc<dyn HttpAccessLogRepository> = Arc::clone(&repository) as _;
        let gateway_usage: Arc<dyn GatewayApiKeyUsageRepository> = Arc::clone(&repository) as _;
        let upstream_usage: Arc<dyn UpstreamCredentialUsageRepository> = repository;
        let capacity = usize::try_from(MAX_TELEMETRY_QUEUE_CAPACITY)
            .expect("telemetry queue maximum fits usize");
        let (sender, receiver) = mpsc::channel(capacity);
        let counters = TelemetryCounters::default();
        let policy = Arc::new(RwLock::new(RequestLogPolicy::from_settings(
            revision, settings,
        )));
        let changes = LogChangeNotifier::new();
        let prune_wakeup = Arc::new(Notify::new());
        let request_prune_wakeup = Arc::new(Notify::new());
        let worker = lifecycle.spawn_tracked(worker::run(
            receiver,
            Arc::clone(&request_logs),
            Arc::clone(&http_access_logs),
            Arc::clone(&gateway_usage),
            worker::WorkerState {
                counters: counters.clone(),
                policy: Arc::clone(&policy),
                changes: changes.clone(),
                prune_wakeup: Arc::clone(&prune_wakeup),
                request_prune_wakeup,
            },
        ));
        Self {
            request_logs: Some(request_logs),
            http_access_logs: Some(http_access_logs),
            gateway_usage_repository: Some(gateway_usage),
            upstream_usage_repository: Some(upstream_usage),
            sender: RwLock::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
            counters,
            policy,
            prune_wakeup,
            gateway_usage: Mutex::new(GatewayUsageTracker::default()),
            changes,
        }
    }

    pub(crate) fn policy(
        &self,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) -> RequestLogPolicy {
        let mut policy = RequestLogPolicy::from_settings(revision, settings);
        if self.request_logs.is_none() {
            policy.enabled = false;
        }
        policy
    }

    fn update_policy(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        if self.request_logs.is_none() {
            return;
        }
        let next = RequestLogPolicy::from_settings(revision, settings);
        let mut current = self.policy.write().expect("request telemetry policy");
        if next.revision > current.revision {
            let cleanup_changed = next.cleanup_limits_differ(*current);
            *current = next;
            drop(current);
            if cleanup_changed {
                self.prune_wakeup.notify_one();
            }
        }
    }

    pub(crate) fn try_record(&self, record: CompletedRequestLog, policy: RequestLogPolicy) {
        if !policy.enabled {
            return;
        }
        self.try_send_event(
            TelemetryEvent::RequestLog(Box::new(record)),
            policy.queue_capacity,
            policy.queue_max_bytes,
        );
    }

    pub fn try_record_http_access(
        &self,
        record: HttpAccessLog,
        settings: &LoggingSettings,
        notification: HttpAccessLogChangeNotification,
    ) {
        let policy = self.policy(record.config_revision, settings);
        if !policy.enabled {
            return;
        }
        self.try_send_event(
            TelemetryEvent::HttpAccessLog {
                record: Box::new(record),
                notification,
            },
            policy.queue_capacity,
            policy.queue_max_bytes,
        );
    }

    pub fn record_gateway_key_use(&self, id: GatewayApiKeyId, revision: ConfigRevision) {
        if self.gateway_usage_repository.is_none() {
            return;
        }
        let used_at = utc_timestamp();
        let should_enqueue = {
            self.gateway_usage
                .lock()
                .expect("gateway usage state")
                .observe(id, revision, used_at.clone(), Instant::now())
        };
        if !should_enqueue {
            return;
        }
        let policy = *self.policy.read().expect("request telemetry policy");
        self.try_send_event(
            TelemetryEvent::GatewayKeyLastUsed {
                id,
                last_used_at: used_at,
            },
            policy.queue_capacity,
            policy.queue_max_bytes,
        );
    }

    #[must_use]
    pub fn gateway_key_last_used_at(&self, id: GatewayApiKeyId) -> Option<String> {
        self.gateway_usage
            .lock()
            .expect("gateway usage state")
            .last_used_at(id)
    }

    pub fn metrics(&self) -> RequestTelemetryMetrics {
        self.counters.snapshot()
    }

    #[must_use]
    pub fn subscribe_request_log_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe_request_logs()
    }

    #[must_use]
    pub fn subscribe_http_access_log_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.changes.subscribe_http_access_logs()
    }

    #[cfg(test)]
    pub(crate) fn current_policy(&self) -> RequestLogPolicy {
        *self.policy.read().expect("request telemetry policy")
    }

    #[cfg(test)]
    pub(crate) fn reconcile_policy_for_test(
        &self,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) {
        self.update_policy(revision, settings);
    }

    pub async fn list(
        &self,
        since_ms: u64,
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<RequestLog>, StorageError> {
        match &self.request_logs {
            Some(repository) => repository.list_request_logs(since_ms, cursor, limit).await,
            None => Ok(LogPage::empty()),
        }
    }

    pub async fn get(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError> {
        match &self.request_logs {
            Some(repository) => repository.get_request_log(request_id).await,
            None => Ok(None),
        }
    }

    pub async fn list_http_access_logs(
        &self,
        since_ms: u64,
        cursor: Option<LogPageCursor>,
        limit: u32,
    ) -> Result<LogPage<HttpAccessLogSummary>, StorageError> {
        match &self.http_access_logs {
            Some(repository) => {
                repository
                    .list_http_access_logs(since_ms, cursor, limit)
                    .await
            }
            None => Ok(LogPage::empty()),
        }
    }

    pub async fn get_http_access_log(
        &self,
        request_id: RequestId,
    ) -> Result<Option<HttpAccessLog>, StorageError> {
        match &self.http_access_logs {
            Some(repository) => repository.get_http_access_log(request_id).await,
            None => Ok(None),
        }
    }

    pub async fn clear_http_access_logs(&self) -> Result<u64, RequestTelemetryControlError> {
        if self.http_access_logs.is_none() {
            return Ok(0);
        }
        let sender = self
            .sender
            .read()
            .expect("request telemetry sender")
            .clone()
            .ok_or(RequestTelemetryControlError::WriterUnavailable)?;
        let permit = sender
            .reserve()
            .await
            .map_err(|_| RequestTelemetryControlError::WriterUnavailable)?;
        let (reply, result) = tokio::sync::oneshot::channel();
        self.counters.reserve_control_slot();
        permit.send(TelemetryEnvelope::new(
            TelemetryEvent::ClearHttpAccessLogs { reply },
        ));
        result
            .await
            .map_err(|_| RequestTelemetryControlError::WriterUnavailable)?
            .map_err(RequestTelemetryControlError::Storage)
    }

    pub async fn gateway_key_usage(&self) -> Result<Vec<GatewayApiKeyUsageSummary>, StorageError> {
        match &self.gateway_usage_repository {
            Some(repository) => repository.list_gateway_api_key_usage().await,
            None => Ok(Vec::new()),
        }
    }

    pub async fn upstream_credential_usage(
        &self,
    ) -> Result<Vec<UpstreamCredentialUsageSummary>, StorageError> {
        match &self.upstream_usage_repository {
            Some(repository) => repository.list_upstream_credential_usage().await,
            None => Ok(Vec::new()),
        }
    }

    pub async fn overview_usage(
        &self,
        range: RequestLogOverviewRange,
    ) -> Result<RequestLogOverview, StorageError> {
        match &self.request_logs {
            Some(repository) => repository.request_log_overview(range).await,
            None => Ok(RequestLogOverview::empty(range, unix_now_ms()?)),
        }
    }

    pub async fn shutdown(&self, wait: Duration) {
        self.sender
            .write()
            .expect("request telemetry sender")
            .take();
        let worker = self.worker.lock().expect("request telemetry worker").take();
        if let Some(mut worker) = worker {
            match tokio::time::timeout(wait, &mut worker).await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!(?error, "request telemetry writer task failed");
                }
                Err(_) => {
                    tracing::warn!("request telemetry writer exceeded shutdown timeout; aborting");
                    worker.abort();
                    if let Err(error) = worker.await
                        && !error.is_cancelled()
                    {
                        tracing::warn!(?error, "request telemetry writer abort failed");
                    }
                }
            }
            self.counters.writer_stopped();
        }
    }

    fn try_send_event(&self, event: TelemetryEvent, capacity: usize, max_bytes: usize) {
        let envelope = TelemetryEnvelope::new(event);
        let records = envelope.record_count;
        let queue_class = envelope.queue_class;
        let owned_bytes = envelope.owned_bytes;
        let sender = self.sender.read().expect("request telemetry sender");
        let Some(sender) = sender.as_ref() else {
            self.counters.rejected(records);
            return;
        };
        if !self
            .counters
            .try_reserve(capacity, max_bytes, owned_bytes, queue_class)
        {
            self.counters.rejected(records);
            return;
        }
        self.counters.enqueued(records, owned_bytes);
        if sender.try_send(envelope).is_err() {
            self.counters.send_failed(records, owned_bytes, queue_class);
        }
    }
}

fn unix_now_ms() -> Result<u64, StorageError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| StorageError::CorruptTelemetry)
}

impl PublishedSnapshotReconciler for RequestTelemetry {
    fn reconcile(&self, snapshot: &PublishedSnapshot) {
        self.update_policy(snapshot.revision(), snapshot.settings().logging());
        let ids = snapshot
            .gateway_api_keys()
            .keys()
            .iter()
            .map(|key| key.id());
        self.gateway_usage
            .lock()
            .expect("gateway usage state")
            .reconcile(snapshot.revision(), ids);
    }
}
