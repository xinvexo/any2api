use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, GatewayApiKeyId, LoggingSettings,
    MAX_TELEMETRY_QUEUE_CAPACITY, RequestId, RequestLog,
};
use any2api_storage::api::{
    GatewayApiKeyUsageRepository, GatewayApiKeyUsageSummary, RequestLogRepository, StorageError,
};
use tokio::{sync::mpsc, task::JoinHandle};

use super::{
    event::TelemetryEvent,
    gateway_usage::{GatewayUsageTracker, utc_timestamp},
    policy::RequestLogPolicy,
    worker,
};
use crate::{logging_reconciler::LoggingSettingsReconciler, process_lifecycle::ProcessLifecycle};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestTelemetryMetrics {
    pub queued_records: usize,
    pub dropped_records: u64,
    pub persisted_records: u64,
}

pub struct RequestTelemetry {
    request_logs: Option<Arc<dyn RequestLogRepository>>,
    gateway_usage_repository: Option<Arc<dyn GatewayApiKeyUsageRepository>>,
    sender: RwLock<Option<mpsc::Sender<TelemetryEvent>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    queued: Arc<AtomicUsize>,
    dropped: Arc<AtomicU64>,
    persisted: Arc<AtomicU64>,
    policy: Arc<RwLock<RequestLogPolicy>>,
    gateway_usage: Mutex<GatewayUsageTracker>,
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
            gateway_usage_repository: None,
            sender: RwLock::new(None),
            worker: Mutex::new(None),
            queued: Arc::new(AtomicUsize::new(0)),
            dropped: Arc::new(AtomicU64::new(0)),
            persisted: Arc::new(AtomicU64::new(0)),
            policy: Arc::new(RwLock::new(policy)),
            gateway_usage: Mutex::new(GatewayUsageTracker::default()),
        }
    }

    pub fn start<R>(
        repository: Arc<R>,
        revision: ConfigRevision,
        settings: &LoggingSettings,
        lifecycle: &ProcessLifecycle,
    ) -> Self
    where
        R: RequestLogRepository + GatewayApiKeyUsageRepository + 'static,
    {
        let request_logs: Arc<dyn RequestLogRepository> = Arc::clone(&repository) as _;
        let gateway_usage: Arc<dyn GatewayApiKeyUsageRepository> = repository;
        let capacity = usize::try_from(MAX_TELEMETRY_QUEUE_CAPACITY)
            .expect("telemetry queue maximum fits usize");
        let (sender, receiver) = mpsc::channel(capacity);
        let queued = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let persisted = Arc::new(AtomicU64::new(0));
        let policy = Arc::new(RwLock::new(RequestLogPolicy::from_settings(
            revision, settings,
        )));
        let worker = lifecycle.spawn_tracked(worker::run(
            receiver,
            Arc::clone(&request_logs),
            Arc::clone(&gateway_usage),
            worker::WorkerState {
                queued: Arc::clone(&queued),
                dropped: Arc::clone(&dropped),
                persisted: Arc::clone(&persisted),
                policy: Arc::clone(&policy),
            },
        ));
        Self {
            request_logs: Some(request_logs),
            gateway_usage_repository: Some(gateway_usage),
            sender: RwLock::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
            queued,
            dropped,
            persisted,
            policy,
            gateway_usage: Mutex::new(GatewayUsageTracker::default()),
        }
    }

    pub(crate) fn policy(
        &self,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) -> RequestLogPolicy {
        let mut next = RequestLogPolicy::from_settings(revision, settings);
        if self.request_logs.is_none() {
            next.enabled = false;
            return next;
        }
        self.update_policy(revision, settings);
        next
    }

    pub(crate) fn update_policy(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        if self.request_logs.is_none() {
            return;
        }
        let next = RequestLogPolicy::from_settings(revision, settings);
        let mut current = self.policy.write().expect("request telemetry policy");
        if next.revision >= current.revision {
            *current = next;
        }
    }

    pub(crate) fn try_record(&self, record: CompletedRequestLog, policy: RequestLogPolicy) {
        if !policy.enabled || !self.reserve_queue_slot(policy.queue_capacity) {
            if policy.enabled {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
        self.send_event(TelemetryEvent::RequestLog(Box::new(record)));
    }

    pub fn record_gateway_key_use(&self, id: GatewayApiKeyId) {
        if self.gateway_usage_repository.is_none() {
            return;
        }
        let used_at = utc_timestamp();
        let should_enqueue = {
            self.gateway_usage
                .lock()
                .expect("gateway usage state")
                .observe(id, used_at.clone(), Instant::now())
        };
        if !should_enqueue {
            return;
        }
        let capacity = self
            .policy
            .read()
            .expect("request telemetry policy")
            .queue_capacity;
        if !self.reserve_queue_slot(capacity) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.send_event(TelemetryEvent::GatewayKeyLastUsed {
            id,
            last_used_at: used_at,
        });
    }

    #[must_use]
    pub fn gateway_key_last_used_at(&self, id: GatewayApiKeyId) -> Option<String> {
        self.gateway_usage
            .lock()
            .expect("gateway usage state")
            .last_used_at(id)
    }

    pub fn metrics(&self) -> RequestTelemetryMetrics {
        RequestTelemetryMetrics {
            queued_records: self.queued.load(Ordering::Acquire),
            dropped_records: self.dropped.load(Ordering::Relaxed),
            persisted_records: self.persisted.load(Ordering::Relaxed),
        }
    }

    #[cfg(test)]
    pub(crate) fn current_policy(&self) -> RequestLogPolicy {
        *self.policy.read().expect("request telemetry policy")
    }

    pub async fn list(&self, limit: u32) -> Result<Vec<RequestLog>, StorageError> {
        match &self.request_logs {
            Some(repository) => repository.list_request_logs(limit).await,
            None => Ok(Vec::new()),
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

    pub async fn gateway_key_usage(&self) -> Result<Vec<GatewayApiKeyUsageSummary>, StorageError> {
        match &self.gateway_usage_repository {
            Some(repository) => repository.list_gateway_api_key_usage().await,
            None => Ok(Vec::new()),
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
                    self.queued.store(0, Ordering::Release);
                }
            }
        }
    }

    fn send_event(&self, event: TelemetryEvent) {
        let sender = self
            .sender
            .read()
            .expect("request telemetry sender")
            .clone();
        let sent = sender.is_some_and(|sender| sender.try_send(event).is_ok());
        if !sent {
            self.queued.fetch_sub(1, Ordering::AcqRel);
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn reserve_queue_slot(&self, capacity: usize) -> bool {
        let mut current = self.queued.load(Ordering::Acquire);
        loop {
            if current >= capacity {
                return false;
            }
            match self.queued.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

impl LoggingSettingsReconciler for RequestTelemetry {
    fn reconcile(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        self.update_policy(revision, settings);
    }
}
