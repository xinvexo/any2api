use any2api_domain::ProviderKind;
use any2api_runtime::api::{
    BalancingProviderSnapshot, BalancingRuntimeSnapshot, BalancingTotalsSnapshot, PublishedSnapshot,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct BalancingRuntimeResponse {
    config_revision: u64,
    scheduler_epoch: u64,
    queue: QueueResponse,
    totals: TotalsResponse,
    providers: Vec<ProviderResponse>,
}

impl BalancingRuntimeResponse {
    pub(crate) fn new(published: &PublishedSnapshot, runtime: &BalancingRuntimeSnapshot) -> Self {
        Self {
            config_revision: published.revision().get(),
            scheduler_epoch: runtime.scheduler_epoch(),
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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
