use std::{
    mem,
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{CompletedRequestLog, HttpAccessLog};
use any2api_storage::api::{
    GatewayApiKeyLastUsedUpdate, GatewayApiKeyUsageRepository, HttpAccessLogRepository,
    RequestLogRepository,
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

#[derive(Default)]
struct WriteBatch {
    request_logs: Vec<CompletedRequestLog>,
    http_access_logs: Vec<HttpAccessLog>,
    gateway_usage: Vec<GatewayApiKeyLastUsedUpdate>,
}

pub(super) async fn run(
    mut receiver: mpsc::Receiver<TelemetryEvent>,
    request_logs: Arc<dyn RequestLogRepository>,
    http_access_logs: Arc<dyn HttpAccessLogRepository>,
    gateway_usage: Arc<dyn GatewayApiKeyUsageRepository>,
    state: WorkerState,
) {
    let mut prune_interval = tokio::time::interval(PRUNE_INTERVAL);
    prune_interval.tick().await;
    prune(request_logs.as_ref(), http_access_logs.as_ref(), &state).await;
    loop {
        tokio::select! {
            _ = prune_interval.tick() => {
                prune(request_logs.as_ref(), http_access_logs.as_ref(), &state).await;
            }
            first = receiver.recv() => {
                let Some(first) = first else {
                    break;
                };
                let mut batch = WriteBatch::default();
                receive_event(
                    first,
                    &mut batch,
                    request_logs.as_ref(),
                    http_access_logs.as_ref(),
                    gateway_usage.as_ref(),
                    &state,
                ).await;
                for _ in 1..WRITE_BATCH_SIZE {
                    let event = match receiver.try_recv() {
                        Ok(event) => event,
                        Err(mpsc::error::TryRecvError::Empty)
                        | Err(mpsc::error::TryRecvError::Disconnected) => break,
                    };
                    receive_event(
                        event,
                        &mut batch,
                        request_logs.as_ref(),
                        http_access_logs.as_ref(),
                        gateway_usage.as_ref(),
                        &state,
                    ).await;
                }
                flush(
                    &mut batch,
                    request_logs.as_ref(),
                    http_access_logs.as_ref(),
                    gateway_usage.as_ref(),
                    &state,
                ).await;
            }
        }
    }
    prune(request_logs.as_ref(), http_access_logs.as_ref(), &state).await;
}

async fn receive_event(
    event: TelemetryEvent,
    batch: &mut WriteBatch,
    request_logs: &dyn RequestLogRepository,
    http_access_logs: &dyn HttpAccessLogRepository,
    gateway_usage: &dyn GatewayApiKeyUsageRepository,
    state: &WorkerState,
) {
    state.queued.fetch_sub(1, Ordering::AcqRel);
    match event {
        TelemetryEvent::RequestLog(record) => batch.request_logs.push(*record),
        TelemetryEvent::HttpAccessLog(record) => batch.http_access_logs.push(*record),
        TelemetryEvent::GatewayKeyLastUsed { id, last_used_at } => {
            batch
                .gateway_usage
                .push(GatewayApiKeyLastUsedUpdate { id, last_used_at });
        }
        TelemetryEvent::ClearHttpAccessLogs { reply } => {
            flush(batch, request_logs, http_access_logs, gateway_usage, state).await;
            let result = http_access_logs.clear_http_access_logs().await;
            let _ = reply.send(result);
        }
    }
}

async fn flush(
    batch: &mut WriteBatch,
    request_logs: &dyn RequestLogRepository,
    http_access_logs: &dyn HttpAccessLogRepository,
    gateway_usage: &dyn GatewayApiKeyUsageRepository,
    state: &WorkerState,
) {
    flush_request_logs(request_logs, state, mem::take(&mut batch.request_logs)).await;
    flush_http_access_logs(
        http_access_logs,
        state,
        mem::take(&mut batch.http_access_logs),
    )
    .await;
    flush_gateway_usage(gateway_usage, state, mem::take(&mut batch.gateway_usage)).await;
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

async fn flush_http_access_logs(
    repository: &dyn HttpAccessLogRepository,
    state: &WorkerState,
    batch: Vec<HttpAccessLog>,
) {
    if batch.is_empty() {
        return;
    }
    match repository.append_http_access_logs(&batch).await {
        Ok(()) => {
            state
                .persisted
                .fetch_add(batch.len() as u64, Ordering::Relaxed);
        }
        Err(error) => {
            state
                .dropped
                .fetch_add(batch.len() as u64, Ordering::Relaxed);
            tracing::warn!(%error, records = batch.len(), "HTTP access log batch was dropped");
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

async fn prune(
    request_logs: &dyn RequestLogRepository,
    http_access_logs: &dyn HttpAccessLogRepository,
    state: &WorkerState,
) {
    let policy = *state.policy.read().expect("request telemetry policy");
    let cutoff = unix_time_ms().saturating_sub(policy.retention_secs.saturating_mul(1_000));
    if let Err(error) = request_logs
        .prune_request_logs(cutoff, policy.max_rows, PRUNE_BATCH_SIZE)
        .await
    {
        tracing::warn!(%error, "request telemetry retention cleanup failed");
    }
    if let Err(error) = http_access_logs
        .prune_http_access_logs(cutoff, policy.max_rows, PRUNE_BATCH_SIZE)
        .await
    {
        tracing::warn!(%error, "HTTP access log retention cleanup failed");
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}
