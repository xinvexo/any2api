use any2api_domain::ProviderKind;
use any2api_runtime::api::{
    BalancingProviderSnapshot, BalancingRuntimeSnapshot, BalancingTotalsSnapshot, ProcessLifecycle,
    PublishedSnapshot, RequestTelemetryMetrics, TransportRuntimeSnapshot,
};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct BalancingRuntimeResponse {
    config_revision: u64,
    scheduler_epoch: u64,
    process: ProcessResponse,
    transport: Option<TransportResponse>,
    breakers: BreakerResponse,
    telemetry: TelemetryResponse,
    queue: QueueResponse,
    totals: TotalsResponse,
    providers: Vec<ProviderResponse>,
}

impl BalancingRuntimeResponse {
    pub(crate) fn new(
        published: &PublishedSnapshot,
        runtime: &BalancingRuntimeSnapshot,
        lifecycle: &ProcessLifecycle,
        transport: Option<TransportRuntimeSnapshot>,
        telemetry: RequestTelemetryMetrics,
    ) -> Self {
        let breakers = runtime.breakers();
        Self {
            config_revision: published.revision().get(),
            scheduler_epoch: runtime.scheduler_epoch(),
            process: ProcessResponse {
                active_requests: lifecycle.active_requests().saturating_sub(1),
                background_tasks: lifecycle.background_task_count(),
                shutdown_phase: lifecycle.phase().as_str(),
            },
            transport: transport.map(TransportResponse::from),
            breakers: BreakerResponse {
                closed: breakers.closed(),
                open: breakers.open(),
                half_open: breakers.half_open(),
            },
            telemetry: TelemetryResponse {
                queued: telemetry.queued_records,
                in_flight: telemetry.in_flight_records,
                capacity: published.settings().logging().telemetry_queue_capacity(),
                dropped: telemetry.dropped_records,
            },
            queue: QueueResponse::from(runtime),
            totals: TotalsResponse::from(runtime.totals()),
            providers: runtime
                .providers()
                .iter()
                .copied()
                .map(ProviderResponse::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct ProcessResponse {
    active_requests: usize,
    background_tasks: usize,
    shutdown_phase: &'static str,
}

#[derive(Clone, Debug, Serialize)]
struct TransportResponse {
    cache_entries: usize,
    cache_capacity: usize,
    cache_hits: u64,
    cache_misses: u64,
    cache_evictions: u64,
}

impl From<TransportRuntimeSnapshot> for TransportResponse {
    fn from(value: TransportRuntimeSnapshot) -> Self {
        Self {
            cache_entries: value.cache_entries(),
            cache_capacity: value.cache_capacity(),
            cache_hits: value.cache_hits(),
            cache_misses: value.cache_misses(),
            cache_evictions: value.cache_evictions(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct BreakerResponse {
    closed: usize,
    open: usize,
    half_open: usize,
}

#[derive(Clone, Debug, Serialize)]
struct TelemetryResponse {
    queued: usize,
    in_flight: usize,
    capacity: u64,
    dropped: u64,
}

#[derive(Clone, Debug, Serialize)]
struct QueueResponse {
    waiting: u32,
    max_waiting: u32,
    timeout_secs: u64,
    on_rate_limited: &'static str,
    fallback_on_rate_limit: bool,
}

impl From<&BalancingRuntimeSnapshot> for QueueResponse {
    fn from(value: &BalancingRuntimeSnapshot) -> Self {
        let queue = value.queue();
        Self {
            waiting: queue.waiting(),
            max_waiting: queue.max_waiting(),
            timeout_secs: queue.timeout_secs(),
            on_rate_limited: if queue.rejects_when_rate_limited() {
                "reject"
            } else {
                "wait"
            },
            fallback_on_rate_limit: queue.fallback_on_rate_limit(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct TotalsResponse {
    credential_count: usize,
    enabled_credential_count: usize,
    limited_credential_count: usize,
    rate_limited_credential_count: usize,
    in_flight: u64,
    requests_in_window: u64,
    fixed_waiters: u64,
    selected: u64,
}

impl From<BalancingTotalsSnapshot> for TotalsResponse {
    fn from(value: BalancingTotalsSnapshot) -> Self {
        Self {
            credential_count: value.credential_count(),
            enabled_credential_count: value.enabled_credential_count(),
            limited_credential_count: value.limited_credential_count(),
            rate_limited_credential_count: value.rate_limited_credential_count(),
            in_flight: value.in_flight(),
            requests_in_window: value.requests_in_window(),
            fixed_waiters: value.fixed_waiters(),
            selected: value.selected(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct ProviderResponse {
    provider_kind: ProviderKind,
    credential_count: usize,
    enabled_credential_count: usize,
    limited_credential_count: usize,
    rate_limited_credential_count: usize,
    in_flight: u64,
    requests_in_window: u64,
    fixed_waiters: u64,
    selected: u64,
}

impl From<BalancingProviderSnapshot> for ProviderResponse {
    fn from(value: BalancingProviderSnapshot) -> Self {
        Self {
            provider_kind: value.provider_kind(),
            credential_count: value.credential_count(),
            enabled_credential_count: value.enabled_credential_count(),
            limited_credential_count: value.limited_credential_count(),
            rate_limited_credential_count: value.rate_limited_credential_count(),
            in_flight: value.in_flight(),
            requests_in_window: value.requests_in_window(),
            fixed_waiters: value.fixed_waiters(),
            selected: value.selected(),
        }
    }
}
