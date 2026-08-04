use std::{
    mem,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{CompletedRequestLog, HttpAccessLog};
use any2api_storage::api::{
    GatewayApiKeyLastUsedUpdate, GatewayApiKeyUsageRepository, HttpAccessLogCapacity,
    HttpAccessLogRepository, REQUEST_LOG_CLEANUP_BATCH_ROWS, RequestLogRepository,
};
use tokio::sync::{Notify, mpsc};

use super::{
    RequestLogPolicy, changes::LogChangeNotifier, event::TelemetryEvent, metrics::TelemetryCounters,
};

const WRITE_BATCH_SIZE: usize = 64;
const PRUNE_INTERVAL: Duration = Duration::from_secs(60);
const INCREMENTAL_VACUUM_BYTES_PER_CYCLE: u64 = 16 * 1024 * 1024;

pub(super) struct WorkerState {
    pub(super) counters: TelemetryCounters,
    pub(super) policy: Arc<RwLock<RequestLogPolicy>>,
    pub(super) changes: LogChangeNotifier,
    pub(super) prune_wakeup: Arc<Notify>,
    pub(super) request_prune_wakeup: Arc<Notify>,
}

#[derive(Default)]
struct WriteBatch {
    request_logs: Vec<CompletedRequestLog>,
    http_access_logs: HttpAccessLogBatch,
    gateway_usage: Vec<GatewayApiKeyLastUsedUpdate>,
}

#[derive(Default)]
struct HttpAccessLogBatch {
    records: Vec<HttpAccessLog>,
    notify_changes: bool,
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
            _ = state.prune_wakeup.notified() => {
                prune(request_logs.as_ref(), http_access_logs.as_ref(), &state).await;
            }
            _ = state.request_prune_wakeup.notified() => {
                prune_request_logs(request_logs.as_ref(), &state).await;
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
    state
        .counters
        .received(event.record_count(), event.queue_class());
    match event {
        TelemetryEvent::RequestLog(record) => batch.request_logs.push(*record),
        TelemetryEvent::HttpAccessLog {
            record,
            notification,
        } => {
            batch.http_access_logs.records.push(*record);
            batch.http_access_logs.notify_changes |= notification.should_notify();
        }
        TelemetryEvent::GatewayKeyLastUsed { id, last_used_at } => {
            batch
                .gateway_usage
                .push(GatewayApiKeyLastUsedUpdate { id, last_used_at });
        }
        TelemetryEvent::ClearHttpAccessLogs { reply } => {
            flush(batch, request_logs, http_access_logs, gateway_usage, state).await;
            let result = http_access_logs.clear_http_access_logs().await;
            if result.as_ref().is_ok_and(|deleted| *deleted > 0) {
                state.changes.http_access_logs_changed();
            }
            if result.is_ok()
                && let Err(error) = http_access_logs
                    .reclaim_http_access_log_storage(INCREMENTAL_VACUUM_BYTES_PER_CYCLE)
                    .await
            {
                tracing::warn!(%error, "HTTP access log storage reclaim failed");
            }
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
    let max_rows = state
        .policy
        .read()
        .expect("request telemetry policy")
        .request_max_rows;
    match repository.append_request_logs(&batch, max_rows).await {
        Ok(cleanup) => {
            state.counters.persisted(batch.len());
            state.changes.request_logs_changed();
            if cleanup.has_more() {
                state.request_prune_wakeup.notify_one();
            }
        }
        Err(error) => {
            state.counters.storage_failed(batch.len());
            tracing::warn!(%error, records = batch.len(), "request telemetry batch was dropped");
        }
    }
}

async fn flush_http_access_logs(
    repository: &dyn HttpAccessLogRepository,
    state: &WorkerState,
    batch: HttpAccessLogBatch,
) {
    if batch.records.is_empty() {
        return;
    }
    let policy = *state.policy.read().expect("request telemetry policy");
    let capacity = HttpAccessLogCapacity::new(
        policy.http_access_max_rows,
        policy.http_access_max_exchange_bytes,
    );
    match repository
        .append_http_access_logs(&batch.records, capacity)
        .await
    {
        Ok(deleted) => {
            state.counters.persisted(batch.records.len());
            if batch.notify_changes || deleted > 0 {
                state.changes.http_access_logs_changed();
            }
        }
        Err(error) => {
            state.counters.storage_failed(batch.records.len());
            tracing::warn!(%error, records = batch.records.len(), "HTTP access log batch was dropped");
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
    let count = batch.len();
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
    match repository.touch_gateway_api_key_last_used(&updates).await {
        Ok(()) => {
            state.counters.persisted(count);
        }
        Err(error) => {
            state.counters.storage_failed(count);
            tracing::warn!(%error, records = count, "gateway key last_used_at batch was dropped");
        }
    }
}

async fn prune(
    request_logs: &dyn RequestLogRepository,
    http_access_logs: &dyn HttpAccessLogRepository,
    state: &WorkerState,
) {
    prune_request_logs(request_logs, state).await;
    let policy = *state.policy.read().expect("request telemetry policy");
    let cutoff = unix_time_ms().saturating_sub(policy.retention_secs.saturating_mul(1_000));

    match http_access_logs
        .prune_http_access_logs(
            cutoff,
            HttpAccessLogCapacity::new(
                policy.http_access_max_rows,
                policy.http_access_max_exchange_bytes,
            ),
            REQUEST_LOG_CLEANUP_BATCH_ROWS,
        )
        .await
    {
        Ok(deleted) if deleted > 0 => state.changes.http_access_logs_changed(),
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "HTTP access log retention cleanup failed"),
    }
    if let Err(error) = http_access_logs
        .reclaim_http_access_log_storage(INCREMENTAL_VACUUM_BYTES_PER_CYCLE)
        .await
    {
        tracing::warn!(%error, "HTTP access log storage reclaim failed");
    }
}

async fn prune_request_logs(request_logs: &dyn RequestLogRepository, state: &WorkerState) {
    let policy = *state.policy.read().expect("request telemetry policy");
    let cutoff = unix_time_ms().saturating_sub(policy.retention_secs.saturating_mul(1_000));
    match request_logs
        .prune_request_logs(
            cutoff,
            policy.request_max_rows,
            REQUEST_LOG_CLEANUP_BATCH_ROWS,
        )
        .await
    {
        Ok(cleanup) => {
            if cleanup.deleted_rows() > 0 {
                state.changes.request_logs_changed();
            }
            if cleanup.has_more() {
                state.request_prune_wakeup.notify_one();
            }
        }
        Err(error) => tracing::warn!(%error, "request telemetry retention cleanup failed"),
    }
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}
