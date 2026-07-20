use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::CompletedRequestLog;
use any2api_storage::api::RequestLogRepository;
use tokio::sync::mpsc;

use super::RequestLogPolicy;

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
    mut receiver: mpsc::Receiver<CompletedRequestLog>,
    repository: Arc<dyn RequestLogRepository>,
    state: WorkerState,
) {
    let mut prune_interval = tokio::time::interval(PRUNE_INTERVAL);
    prune_interval.tick().await;
    prune(repository.as_ref(), &state).await;
    loop {
        tokio::select! {
            _ = prune_interval.tick() => {
                prune(repository.as_ref(), &state).await;
            }
            first = receiver.recv() => {
                let Some(first) = first else {
                    break;
                };
                state.queued.fetch_sub(1, Ordering::AcqRel);
                let mut batch = Vec::with_capacity(WRITE_BATCH_SIZE);
                batch.push(first);
                while batch.len() < WRITE_BATCH_SIZE {
                    match receiver.try_recv() {
                        Ok(record) => {
                            state.queued.fetch_sub(1, Ordering::AcqRel);
                            batch.push(record);
                        }
                        Err(mpsc::error::TryRecvError::Empty)
                        | Err(mpsc::error::TryRecvError::Disconnected) => break,
                    }
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
        }
    }
    prune(repository.as_ref(), &state).await;
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
