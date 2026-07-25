use std::collections::BTreeMap;

use any2api_domain::{ProviderEndpointId, ProviderKind, ProxyKind, ProxyProfileId};
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
}

impl TotalsResponse {
    fn from_credentials(credentials: &[CredentialResponse]) -> Self {
        Self {
            credential_count: credentials.len(),
            enabled_credential_count: credentials
                .iter()
                .filter(|item| item.is_schedulable())
                .count(),
            limited_credential_count: credentials
                .iter()
                .filter(|item| item.requests_per_minute.is_some())
                .count(),
            rate_limited_credential_count: credentials
                .iter()
                .filter(|item| item.is_rate_limited())
                .count(),
            in_flight: credentials
                .iter()
                .map(|item| u64::from(item.in_flight))
                .sum(),
            requests_in_window: credentials
                .iter()
                .map(|item| u64::from(item.requests_in_window))
                .sum(),
            fixed_waiters: credentials
                .iter()
                .map(|item| u64::from(item.fixed_waiters))
                .sum(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProviderResponse {
    provider_kind: ProviderKind,
    credential_count: usize,
    limited_credential_count: usize,
    rate_limited_credential_count: usize,
    in_flight: u64,
    requests_in_window: u64,
    selected: u64,
}

impl ProviderResponse {
    fn from_credentials(credentials: &[CredentialResponse]) -> Vec<Self> {
        let mut providers = BTreeMap::<ProviderKind, Self>::new();
        for credential in credentials {
            let provider = providers.entry(credential.provider_kind).or_insert(Self {
                provider_kind: credential.provider_kind,
                credential_count: 0,
                limited_credential_count: 0,
                rate_limited_credential_count: 0,
                in_flight: 0,
                requests_in_window: 0,
                selected: 0,
            });
            provider.credential_count += 1;
            provider.in_flight += u64::from(credential.in_flight);
            provider.requests_in_window += u64::from(credential.requests_in_window);
            if credential.requests_per_minute.is_some() {
                provider.limited_credential_count += 1;
            }
            if credential.is_rate_limited() {
                provider.rate_limited_credential_count += 1;
            }
            provider.selected += credential.counters.selected;
        }
        providers.into_values().collect()
    }
}

#[derive(Debug, Serialize)]
struct CredentialResponse {
    credential_id: String,
    credential_source: &'static str,
    label: String,
    enabled: bool,
    authentication_expired: bool,
    provider_kind: ProviderKind,
    endpoint_id: Option<ProviderEndpointId>,
    endpoint_name: Option<String>,
    endpoint_enabled: bool,
    proxy_id: ProxyProfileId,
    proxy_name: String,
    proxy_kind: ProxyKind,
    proxy_enabled: bool,
    in_flight: u32,
    requests_per_minute: Option<u32>,
    requests_in_window: u32,
    remaining_requests: Option<u32>,
    retry_in_ms: Option<u64>,
    fixed_waiters: u32,
    counters: CountersResponse,
    models: Vec<ModelHealthResponse>,
}

impl CredentialResponse {
    fn new(published: &PublishedSnapshot, runtime: &BalancingCredentialSnapshot) -> Option<Self> {
        let proxy = published.proxies().get(runtime.proxy_id())?;
        Some(Self {
            credential_id: runtime.credential_id().source_uuid().to_string(),
            credential_source: if runtime.oauth_account_id().is_some() {
                "oauth_account"
            } else {
                "provider_credential"
            },
            label: runtime.label().to_owned(),
            enabled: runtime.enabled(),
            authentication_expired: runtime.authentication_expired(),
            provider_kind: runtime.provider_kind(),
            endpoint_id: runtime.provider_endpoint_id(),
            endpoint_name: runtime.endpoint_name().map(str::to_owned),
            endpoint_enabled: runtime.endpoint_enabled(),
            proxy_id: proxy.id(),
            proxy_name: proxy.name().to_owned(),
            proxy_kind: proxy.kind(),
            proxy_enabled: proxy.enabled(),
            in_flight: runtime.in_flight(),
            requests_per_minute: runtime.requests_per_minute(),
            requests_in_window: runtime.requests_in_window(),
            remaining_requests: runtime.remaining_requests(),
            retry_in_ms: runtime.retry_in_ms(),
            fixed_waiters: runtime.fixed_waiters(),
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
        self.enabled && !self.authentication_expired && self.endpoint_enabled && self.proxy_enabled
    }

    const fn is_rate_limited(&self) -> bool {
        matches!(self.remaining_requests, Some(0))
    }
}

#[derive(Debug, Serialize)]
struct CountersResponse {
    selected: u64,
    filtered_rate_limit: u64,
    filtered_credential_health: u64,
    filtered_endpoint_health: u64,
    filtered_proxy_health: u64,
}

impl From<CredentialBalancingCounters> for CountersResponse {
    fn from(value: CredentialBalancingCounters) -> Self {
        Self {
            selected: value.selected(),
            filtered_rate_limit: value.filtered_rate_limit(),
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
