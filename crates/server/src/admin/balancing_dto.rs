use std::collections::BTreeMap;

use any2api_domain::{CredentialId, ProviderEndpointId, ProviderKind, ProxyKind, ProxyProfileId};
use any2api_runtime::api::{
    BalancingCredentialModelSnapshot, BalancingCredentialSnapshot, BalancingHealthStatus,
    BalancingRuntimeSnapshot, CredentialBalancingCounters, PublishedSnapshot,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct BalancingRuntimeResponse {
    config_revision: u64,
    scheduler_epoch: u64,
    queue: QueueResponse,
    auxiliary: AuxiliaryResponse,
    totals: TotalsResponse,
    providers: Vec<ProviderResponse>,
    credentials: Vec<CredentialResponse>,
}

impl BalancingRuntimeResponse {
    pub(crate) fn new(published: &PublishedSnapshot, runtime: &BalancingRuntimeSnapshot) -> Self {
        let credentials = runtime
            .credentials()
            .iter()
            .filter_map(|credential| CredentialResponse::new(published, credential))
            .collect::<Vec<_>>();
        Self {
            config_revision: published.revision().get(),
            scheduler_epoch: runtime.scheduler_epoch(),
            queue: QueueResponse::from(runtime),
            auxiliary: AuxiliaryResponse::from(runtime),
            totals: TotalsResponse::from_credentials(&credentials),
            providers: ProviderResponse::from_credentials(&credentials),
            credentials,
        }
    }
}

#[derive(Debug, Serialize)]
struct QueueResponse {
    waiting: u32,
    max_waiting: u32,
    timeout_secs: u64,
    on_saturated: &'static str,
    fallback_on_saturation: bool,
}

impl From<&BalancingRuntimeSnapshot> for QueueResponse {
    fn from(value: &BalancingRuntimeSnapshot) -> Self {
        let queue = value.queue();
        Self {
            waiting: queue.waiting(),
            max_waiting: queue.max_waiting(),
            timeout_secs: queue.timeout_secs(),
            on_saturated: if queue.rejects_when_saturated() {
                "reject"
            } else {
                "wait"
            },
            fallback_on_saturation: queue.fallback_on_saturation(),
        }
    }
}

#[derive(Debug, Serialize)]
struct AuxiliaryResponse {
    in_flight: u32,
    max_global: u32,
    max_per_credential: u32,
}

impl From<&BalancingRuntimeSnapshot> for AuxiliaryResponse {
    fn from(value: &BalancingRuntimeSnapshot) -> Self {
        let auxiliary = value.auxiliary();
        Self {
            in_flight: auxiliary.in_flight(),
            max_global: auxiliary.max_global(),
            max_per_credential: auxiliary.max_per_credential(),
        }
    }
}

#[derive(Debug, Serialize)]
struct TotalsResponse {
    credential_count: usize,
    enabled_credential_count: usize,
    in_flight: u64,
    max_concurrency: u64,
    fixed_waiters: u64,
    auxiliary_in_flight: u64,
}

impl TotalsResponse {
    fn from_credentials(credentials: &[CredentialResponse]) -> Self {
        Self {
            credential_count: credentials.len(),
            enabled_credential_count: credentials
                .iter()
                .filter(|item| item.is_schedulable())
                .count(),
            in_flight: credentials
                .iter()
                .map(|item| u64::from(item.in_flight))
                .sum(),
            max_concurrency: credentials
                .iter()
                .filter(|item| item.is_schedulable())
                .map(|item| u64::from(item.max_concurrency))
                .sum(),
            fixed_waiters: credentials
                .iter()
                .map(|item| u64::from(item.fixed_waiters))
                .sum(),
            auxiliary_in_flight: credentials
                .iter()
                .map(|item| u64::from(item.auxiliary_in_flight))
                .sum(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProviderResponse {
    provider_kind: ProviderKind,
    credential_count: usize,
    in_flight: u64,
    max_concurrency: u64,
    selected_generation: u64,
    selected_auxiliary: u64,
}

impl ProviderResponse {
    fn from_credentials(credentials: &[CredentialResponse]) -> Vec<Self> {
        let mut providers = BTreeMap::<ProviderKind, Self>::new();
        for credential in credentials {
            let provider = providers.entry(credential.provider_kind).or_insert(Self {
                provider_kind: credential.provider_kind,
                credential_count: 0,
                in_flight: 0,
                max_concurrency: 0,
                selected_generation: 0,
                selected_auxiliary: 0,
            });
            provider.credential_count += 1;
            provider.in_flight += u64::from(credential.in_flight);
            if credential.is_schedulable() {
                provider.max_concurrency += u64::from(credential.max_concurrency);
            }
            provider.selected_generation += credential.counters.selected_generation;
            provider.selected_auxiliary += credential.counters.selected_auxiliary;
        }
        providers.into_values().collect()
    }
}

#[derive(Debug, Serialize)]
struct CredentialResponse {
    credential_id: CredentialId,
    label: String,
    enabled: bool,
    provider_kind: ProviderKind,
    endpoint_id: ProviderEndpointId,
    endpoint_name: String,
    endpoint_enabled: bool,
    proxy_id: ProxyProfileId,
    proxy_name: String,
    proxy_kind: ProxyKind,
    proxy_enabled: bool,
    in_flight: u32,
    max_concurrency: u32,
    fixed_waiters: u32,
    auxiliary_in_flight: u32,
    counters: CountersResponse,
    models: Vec<ModelHealthResponse>,
}

impl CredentialResponse {
    fn new(published: &PublishedSnapshot, runtime: &BalancingCredentialSnapshot) -> Option<Self> {
        let credential = published
            .provider_credentials()
            .get(runtime.credential_id())?;
        let endpoint = published.provider_endpoints().get(runtime.endpoint_id())?;
        let proxy = published.proxies().get(runtime.proxy_id())?;
        Some(Self {
            credential_id: credential.id(),
            label: credential.label().to_owned(),
            enabled: credential.enabled(),
            provider_kind: endpoint.provider_kind(),
            endpoint_id: endpoint.id(),
            endpoint_name: endpoint.name().to_owned(),
            endpoint_enabled: endpoint.enabled(),
            proxy_id: proxy.id(),
            proxy_name: proxy.name().to_owned(),
            proxy_kind: proxy.kind(),
            proxy_enabled: proxy.enabled(),
            in_flight: runtime.in_flight(),
            max_concurrency: runtime.max_concurrency(),
            fixed_waiters: runtime.fixed_waiters(),
            auxiliary_in_flight: runtime.auxiliary_in_flight(),
            counters: CountersResponse::from(runtime.counters()),
            models: runtime
                .models()
                .iter()
                .map(ModelHealthResponse::from)
                .collect(),
        })
    }
}

impl CredentialResponse {
    const fn is_schedulable(&self) -> bool {
        self.enabled && self.endpoint_enabled && self.proxy_enabled
    }
}

#[derive(Debug, Serialize)]
struct CountersResponse {
    selected_generation: u64,
    selected_auxiliary: u64,
    filtered_capacity: u64,
    filtered_credential_health: u64,
    filtered_endpoint_health: u64,
    filtered_proxy_health: u64,
}

impl From<CredentialBalancingCounters> for CountersResponse {
    fn from(value: CredentialBalancingCounters) -> Self {
        Self {
            selected_generation: value.selected_generation(),
            selected_auxiliary: value.selected_auxiliary(),
            filtered_capacity: value.filtered_capacity(),
            filtered_credential_health: value.filtered_credential_health(),
            filtered_endpoint_health: value.filtered_endpoint_health(),
            filtered_proxy_health: value.filtered_proxy_health(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ModelHealthResponse {
    upstream_model: String,
    credential: HealthResponse,
    endpoint: HealthResponse,
    proxy: HealthResponse,
}

impl From<&BalancingCredentialModelSnapshot> for ModelHealthResponse {
    fn from(value: &BalancingCredentialModelSnapshot) -> Self {
        Self {
            upstream_model: value.upstream_model().to_owned(),
            credential: HealthResponse::from(value.credential()),
            endpoint: HealthResponse::from(value.endpoint()),
            proxy: HealthResponse::from(value.proxy()),
        }
    }
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    retry_in_ms: Option<u64>,
}

impl From<BalancingHealthStatus> for HealthResponse {
    fn from(value: BalancingHealthStatus) -> Self {
        match value {
            BalancingHealthStatus::Available => Self {
                status: "available",
                retry_in_ms: None,
            },
            BalancingHealthStatus::Cooling { retry_in_ms } => Self {
                status: "cooling",
                retry_in_ms: Some(retry_in_ms),
            },
            BalancingHealthStatus::Unavailable => Self {
                status: "unavailable",
                retry_in_ms: None,
            },
        }
    }
}
