use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use any2api_payload_buffer::payload_buffer_metrics;
use any2api_runtime::api::{
    PublicRequestService, RequestTelemetry, RuntimeRegistry, SnapshotStore, SystemMetricsError,
};
use serde::Serialize;
use tokio::{
    sync::watch,
    time::{self, MissedTickBehavior},
};

use super::{balancing::BalancingRuntimeResponse, overview::OverviewResourcesResponse};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const INITIAL_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(3);

/// Shared process-local source for administrator realtime state.
///
/// The sampler is started only when an endpoint needs it, then remains shared
/// across all SSE connections and overview resource reads.
pub(crate) struct AdminRealtimeHub {
    runtime: Arc<RuntimeRegistry>,
    snapshots: Arc<SnapshotStore>,
    public_requests: Arc<PublicRequestService>,
    telemetry: Arc<RequestTelemetry>,
    latest: watch::Sender<Option<Arc<OverviewSnapshot>>>,
    started: AtomicBool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OverviewSnapshot {
    sampled_at_ms: u64,
    resources: OverviewResourcesResponse,
    runtime: BalancingRuntimeResponse,
    freshness: &'static str,
    error: Option<&'static str>,
}

impl OverviewSnapshot {
    fn fresh(
        sampled_at_ms: u64,
        resources: OverviewResourcesResponse,
        runtime: BalancingRuntimeResponse,
    ) -> Self {
        Self {
            sampled_at_ms,
            resources,
            runtime,
            freshness: "fresh",
            error: None,
        }
    }

    fn stale(&self) -> Self {
        Self {
            sampled_at_ms: self.sampled_at_ms,
            resources: self.resources.clone(),
            runtime: self.runtime.clone(),
            freshness: "stale",
            error: Some("system_metrics_unavailable"),
        }
    }

    pub(crate) fn resources(&self) -> OverviewResourcesResponse {
        self.resources.clone()
    }
}

impl AdminRealtimeHub {
    pub(crate) fn new(
        runtime: Arc<RuntimeRegistry>,
        snapshots: Arc<SnapshotStore>,
        public_requests: Arc<PublicRequestService>,
        telemetry: Arc<RequestTelemetry>,
    ) -> Self {
        let (latest, _) = watch::channel(None);
        Self {
            runtime,
            snapshots,
            public_requests,
            telemetry,
            latest,
            started: AtomicBool::new(false),
        }
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<Option<Arc<OverviewSnapshot>>> {
        self.start();
        self.latest.subscribe()
    }

    pub(crate) async fn current_snapshot(&self) -> Option<Arc<OverviewSnapshot>> {
        let mut receiver = self.subscribe();
        let wait = async move {
            if let Some(snapshot) = receiver.borrow().clone() {
                return Some(snapshot);
            }
            loop {
                receiver.changed().await.ok()?;
                if let Some(snapshot) = receiver.borrow().clone() {
                    return Some(snapshot);
                }
            }
        };
        time::timeout(INITIAL_SNAPSHOT_TIMEOUT, wait)
            .await
            .ok()
            .flatten()
    }

    fn start(&self) {
        if self
            .started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let lifecycle = self.runtime.lifecycle();
        let sender = self.latest.clone();
        let runtime = Arc::clone(&self.runtime);
        let snapshots = Arc::clone(&self.snapshots);
        let public_requests = Arc::clone(&self.public_requests);
        let telemetry = Arc::clone(&self.telemetry);
        drop(lifecycle.spawn_until_draining(async move {
            run_sampler(sender, runtime, snapshots, public_requests, telemetry).await;
        }));
    }
}

async fn run_sampler(
    sender: watch::Sender<Option<Arc<OverviewSnapshot>>>,
    runtime: Arc<RuntimeRegistry>,
    snapshots: Arc<SnapshotStore>,
    public_requests: Arc<PublicRequestService>,
    telemetry: Arc<RequestTelemetry>,
) {
    let mut ticker = time::interval(SAMPLE_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match sample_once(&runtime, &snapshots, &public_requests, &telemetry).await {
            Ok(snapshot) => {
                sender.send_replace(Some(Arc::new(snapshot)));
            }
            Err(error) => {
                tracing::warn!(%error, "admin realtime overview sampling failed");
                if let Some(previous) = sender.borrow().clone() {
                    sender.send_replace(Some(Arc::new(previous.stale())));
                }
            }
        }
    }
}

async fn sample_once(
    runtime: &RuntimeRegistry,
    snapshots: &SnapshotStore,
    public_requests: &PublicRequestService,
    telemetry: &RequestTelemetry,
) -> Result<OverviewSnapshot, SystemMetricsError> {
    let metrics = runtime.system_metrics_snapshot().await?;
    let published = snapshots.load();
    let runtime_snapshot = runtime.balancing_snapshot(&published);
    let lifecycle = runtime.lifecycle();
    let telemetry_metrics = telemetry.metrics();
    let balancing = BalancingRuntimeResponse::new(
        &published,
        &runtime_snapshot,
        &lifecycle,
        public_requests.transport_runtime_snapshot(),
        telemetry_metrics,
    );
    let resources = OverviewResourcesResponse::new(
        metrics,
        payload_buffer_metrics(),
        telemetry_metrics,
        lifecycle.memory_reclamation_metrics(),
    );
    Ok(OverviewSnapshot::fresh(
        metrics.sampled_at_ms,
        resources,
        balancing,
    ))
}
