use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::CompletedRequestLog;
use any2api_storage::api::{
    GatewayApiKeyLastUsedUpdate, GatewayApiKeyUsageRepository, RequestLogRepository,
};
use tokio::sync::mpsc;

use super::{RequestLogPolicy, event::TelemetryEvent};

const WRITE_BATCH_SIZE: usize = 64;
const PRUNE_BATCH_SIZE: u32 = 1_000;
const PRUNE_INTERVAL: Duration = Duration::from_secs(60);

pub(super) struct WorkerState {
    pub(super) queued: Arc<AtomicUsize>,
    pub(super) dropped: Arc<AtomicU64>,
    pub(super) persisted: Arc<AtomicU64>,
    pub(super) policy: Arc<RwLock<RequestLogPolicy>>,
}

pub(super) async fn run(
    mut receiver: mpsc::Receiver<TelemetryEvent>,
    request_logs: Arc<dyn RequestLogRepository>,
    gateway_usage: Arc<dyn GatewayApiKeyUsageRepository>,
    state: WorkerState,
) {
    let mut prune_interval = tokio::time::interval(PRUNE_INTERVAL);
    prune_interval.tick().await;
    prune(request_logs.as_ref(), &state).await;
    loop {
        tokio::select! {
            _ = prune_interval.tick() => {
                prune(request_logs.as_ref(), &state).await;
            }
            first = receiver.recv() => {
                let Some(first) = first else {
                    break;
                };
                state.queued.fetch_sub(1, Ordering::AcqRel);
                let mut request_batch = Vec::with_capacity(WRITE_BATCH_SIZE);
                let mut usage_batch = Vec::with_capacity(WRITE_BATCH_SIZE);
                push_event(first, &mut request_batch, &mut usage_batch);
                while request_batch.len() + usage_batch.len() < WRITE_BATCH_SIZE {
                    match receiver.try_recv() {
                        Ok(event) => {
                            state.queued.fetch_sub(1, Ordering::AcqRel);
                            push_event(event, &mut request_batch, &mut usage_batch);
                        }
                        Err(mpsc::error::TryRecvError::Empty)
                        | Err(mpsc::error::TryRecvError::Disconnected) => break,
                    }
                }
                flush_request_logs(request_logs.as_ref(), &state, request_batch).await;
                flush_gateway_usage(gateway_usage.as_ref(), &state, usage_batch).await;
            }
        }
    }
    prune(request_logs.as_ref(), &state).await;
}

fn push_event(
    event: TelemetryEvent,
    request_batch: &mut Vec<CompletedRequestLog>,
    usage_batch: &mut Vec<GatewayApiKeyLastUsedUpdate>,
) {
    match event {
        TelemetryEvent::RequestLog(record) => request_batch.push(*record),
        TelemetryEvent::GatewayKeyLastUsed { id, last_used_at } => {
            usage_batch.push(GatewayApiKeyLastUsedUpdate { id, last_used_at });
        }
    }
}

async fn flush_request_logs(
    repository: &dyn RequestLogRepository,
    state: &WorkerState,
    batch: Vec<CompletedRequestLog>,
) {
    if batch.is_empty() {
        return;
    }
    match repository.append_request_logs(&batch).await {
        Ok(()) => {
            state
                .persisted
                .fetch_add(batch.len() as u64, Ordering::Relaxed);
        }
        Err(error) => {
            state
                .dropped
                .fetch_add(batch.len() as u64, Ordering::Relaxed);
            tracing::warn!(%error, records = batch.len(), "request telemetry batch was dropped");
        }
    }
}

async fn flush_gateway_usage(
    repository: &dyn GatewayApiKeyUsageRepository,
    state: &WorkerState,
    batch: Vec<GatewayApiKeyLastUsedUpdate>,
) {
    if batch.is_empty() {
        return;
    }
    // Keep the newest timestamp per key inside one batch.
    let mut collapsed = std::collections::HashMap::new();
    for update in batch {
        collapsed
            .entry(update.id)
            .and_modify(|existing: &mut String| {
                if update.last_used_at.as_str() > existing.as_str() {
                    *existing = update.last_used_at.clone();
                }
            })
            .or_insert(update.last_used_at);
    }
    let updates = collapsed
        .into_iter()
        .map(|(id, last_used_at)| GatewayApiKeyLastUsedUpdate { id, last_used_at })
        .collect::<Vec<_>>();
    let count = updates.len() as u64;
    match repository.touch_gateway_api_key_last_used(&updates).await {
        Ok(()) => {
            state.persisted.fetch_add(count, Ordering::Relaxed);
        }
        Err(error) => {
            state.dropped.fetch_add(count, Ordering::Relaxed);
            tracing::warn!(%error, records = count, "gateway key last_used_at batch was dropped");
        }
    }
}

async fn prune(repository: &dyn RequestLogRepository, state: &WorkerState) {
    let policy = *state.policy.read().expect("request telemetry policy");
    let cutoff = unix_time_ms().saturating_sub(policy.retention_ms);
    if let Err(error) = repository
        .prune_request_logs(cutoff, policy.max_rows, PRUNE_BATCH_SIZE)
        .await
    {
        tracing::warn!(%error, "request telemetry retention cleanup failed");
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}
