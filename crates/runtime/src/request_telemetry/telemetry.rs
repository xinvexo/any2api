use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::{
    CompletedRequestLog, ConfigRevision, LoggingSettings, MAX_TELEMETRY_QUEUE_CAPACITY, RequestId,
    RequestLog,
};
use any2api_storage::api::{RequestLogRepository, StorageError};
use tokio::{sync::mpsc, task::JoinHandle};

use super::{policy::RequestLogPolicy, worker};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestTelemetryMetrics {
    pub queued_records: usize,
    pub dropped_records: u64,
    pub persisted_records: u64,
}

pub struct RequestTelemetry {
    repository: Option<Arc<dyn RequestLogRepository>>,
    sender: RwLock<Option<mpsc::Sender<CompletedRequestLog>>>,
    worker: Mutex<Option<JoinHandle<()>>>,
    queued: Arc<AtomicUsize>,
    dropped: Arc<AtomicU64>,
    persisted: Arc<AtomicU64>,
    policy: Arc<RwLock<RequestLogPolicy>>,
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
            repository: None,
            sender: RwLock::new(None),
            worker: Mutex::new(None),
            queued: Arc::new(AtomicUsize::new(0)),
            dropped: Arc::new(AtomicU64::new(0)),
            persisted: Arc::new(AtomicU64::new(0)),
            policy: Arc::new(RwLock::new(policy)),
        }
    }

    pub fn start<R>(
        repository: Arc<R>,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) -> Self
    where
        R: RequestLogRepository + 'static,
    {
        let repository: Arc<dyn RequestLogRepository> = repository;
        let capacity = usize::try_from(MAX_TELEMETRY_QUEUE_CAPACITY)
            .expect("telemetry queue maximum fits usize");
        let (sender, receiver) = mpsc::channel(capacity);
        let queued = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicU64::new(0));
        let persisted = Arc::new(AtomicU64::new(0));
        let policy = Arc::new(RwLock::new(RequestLogPolicy::from_settings(
            revision, settings,
        )));
        let worker = tokio::spawn(worker::run(
            receiver,
            Arc::clone(&repository),
            worker::WorkerState {
                queued: Arc::clone(&queued),
                dropped: Arc::clone(&dropped),
                persisted: Arc::clone(&persisted),
                policy: Arc::clone(&policy),
            },
        ));
        Self {
            repository: Some(repository),
            sender: RwLock::new(Some(sender)),
            worker: Mutex::new(Some(worker)),
            queued,
            dropped,
            persisted,
            policy,
        }
    }

    pub(crate) fn policy(
        &self,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) -> RequestLogPolicy {
        let mut next = RequestLogPolicy::from_settings(revision, settings);
        if self.repository.is_none() {
            next.enabled = false;
            return next;
        }
        self.update_policy(revision, settings);
        next
    }

    pub(crate) fn update_policy(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        if self.repository.is_none() {
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
        let sender = self
            .sender
            .read()
            .expect("request telemetry sender")
            .clone();
        let sent = sender.is_some_and(|sender| sender.try_send(record).is_ok());
        if !sent {
            self.queued.fetch_sub(1, Ordering::AcqRel);
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
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
        match &self.repository {
            Some(repository) => repository.list_request_logs(limit).await,
            None => Ok(Vec::new()),
        }
    }

    pub async fn get(
        &self,
        request_id: RequestId,
    ) -> Result<Option<CompletedRequestLog>, StorageError> {
        match &self.repository {
            Some(repository) => repository.get_request_log(request_id).await,
            None => Ok(None),
        }
    }

    pub async fn shutdown(&self, wait: Duration) {
        self.sender
            .write()
            .expect("request telemetry sender")
            .take();
        let worker = self.worker.lock().expect("request telemetry worker").take();
        if let Some(worker) = worker
            && tokio::time::timeout(wait, worker).await.is_err()
        {
            tracing::warn!("request telemetry writer did not stop before shutdown timeout");
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
