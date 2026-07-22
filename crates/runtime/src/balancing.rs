use std::collections::{BTreeSet, HashMap};

use any2api_domain::{CredentialId, ProviderEndpointId, ProxyProfileId};
use tokio::time::Instant;

use crate::{
    credential_runtime::{CredentialBalancingCounters, CredentialRuntimeBinding},
    health::HealthAcquireError,
    published_snapshot::PublishedSnapshot,
    queue::SaturationAction,
    registry::RuntimeRegistry,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalancingRuntimeSnapshot {
    scheduler_epoch: u64,
    queue: BalancingQueueSnapshot,
    auxiliary: BalancingAuxiliarySnapshot,
    credentials: Vec<BalancingCredentialSnapshot>,
}

impl BalancingRuntimeSnapshot {
    pub const fn scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch
    }

    pub const fn queue(&self) -> BalancingQueueSnapshot {
        self.queue
    }

    pub const fn auxiliary(&self) -> BalancingAuxiliarySnapshot {
        self.auxiliary
    }

    pub fn credentials(&self) -> &[BalancingCredentialSnapshot] {
        &self.credentials
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BalancingQueueSnapshot {
    waiting: u32,
    max_waiting: u32,
    timeout_secs: u64,
    rejects_when_saturated: bool,
    fallback_on_saturation: bool,
}

impl BalancingQueueSnapshot {
    pub const fn waiting(self) -> u32 {
        self.waiting
    }

    pub const fn max_waiting(self) -> u32 {
        self.max_waiting
    }

    pub const fn timeout_secs(self) -> u64 {
        self.timeout_secs
    }

    pub const fn rejects_when_saturated(self) -> bool {
        self.rejects_when_saturated
    }

    pub const fn fallback_on_saturation(self) -> bool {
        self.fallback_on_saturation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BalancingAuxiliarySnapshot {
    in_flight: u32,
    max_global: u32,
    max_per_credential: u32,
}

impl BalancingAuxiliarySnapshot {
    pub const fn in_flight(self) -> u32 {
        self.in_flight
    }

    pub const fn max_global(self) -> u32 {
        self.max_global
    }

    pub const fn max_per_credential(self) -> u32 {
        self.max_per_credential
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalancingCredentialSnapshot {
    credential_id: CredentialId,
    endpoint_id: ProviderEndpointId,
    proxy_id: ProxyProfileId,
    in_flight: u32,
    max_concurrency: u32,
    fixed_waiters: u32,
    auxiliary_in_flight: u32,
    counters: CredentialBalancingCounters,
    models: Vec<BalancingCredentialModelSnapshot>,
}

impl BalancingCredentialSnapshot {
    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    pub const fn endpoint_id(&self) -> ProviderEndpointId {
        self.endpoint_id
    }

    pub const fn proxy_id(&self) -> ProxyProfileId {
        self.proxy_id
    }

    pub const fn in_flight(&self) -> u32 {
        self.in_flight
    }

    pub const fn max_concurrency(&self) -> u32 {
        self.max_concurrency
    }

    pub const fn fixed_waiters(&self) -> u32 {
        self.fixed_waiters
    }

    pub const fn auxiliary_in_flight(&self) -> u32 {
        self.auxiliary_in_flight
    }

    pub const fn counters(&self) -> CredentialBalancingCounters {
        self.counters
    }

    pub fn models(&self) -> &[BalancingCredentialModelSnapshot] {
        &self.models
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalancingCredentialModelSnapshot {
    upstream_model: String,
    credential: BalancingHealthStatus,
    endpoint: BalancingHealthStatus,
    proxy: BalancingHealthStatus,
}

impl BalancingCredentialModelSnapshot {
    pub fn upstream_model(&self) -> &str {
        &self.upstream_model
    }

    pub const fn credential(&self) -> BalancingHealthStatus {
        self.credential
    }

    pub const fn endpoint(&self) -> BalancingHealthStatus {
        self.endpoint
    }

    pub const fn proxy(&self) -> BalancingHealthStatus {
        self.proxy
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BalancingHealthStatus {
    Available,
    Cooling { retry_in_ms: u64 },
    Unavailable,
}

pub(crate) fn snapshot(
    runtime: &RuntimeRegistry,
    published: &PublishedSnapshot,
) -> BalancingRuntimeSnapshot {
    let queue_policy = published.queue_policy();
    let (auxiliary_in_flight, auxiliary_limits) =
        published.auxiliary_scheduler().runtime_capacity();
    let models = route_models(published);
    let credentials = published
        .provider_credentials()
        .credentials()
        .iter()
        .filter_map(|credential| {
            let binding = published.credential_runtime(credential.id())?;
            let proxy = published.resolved_proxy_for_credential(credential.id())?;
            Some(credential_snapshot(
                published,
                binding,
                credential.provider_endpoint_id(),
                proxy.id(),
                models.get(&credential.provider_endpoint_id()),
            ))
        })
        .collect();

    BalancingRuntimeSnapshot {
        scheduler_epoch: runtime.scheduler_epoch(),
        queue: BalancingQueueSnapshot {
            waiting: runtime.queue_waiting_count(),
            max_waiting: queue_policy.max_waiting_requests(),
            timeout_secs: queue_policy.queue_timeout().as_secs(),
            rejects_when_saturated: queue_policy.on_saturated() == SaturationAction::Reject,
            fallback_on_saturation: queue_policy.fallback_on_saturation(),
        },
        auxiliary: BalancingAuxiliarySnapshot {
            in_flight: auxiliary_in_flight,
            max_global: auxiliary_limits.global(),
            max_per_credential: auxiliary_limits.per_credential(),
        },
        credentials,
    }
}

fn credential_snapshot(
    published: &PublishedSnapshot,
    binding: &CredentialRuntimeBinding,
    endpoint_id: ProviderEndpointId,
    proxy_id: ProxyProfileId,
    models: Option<&BTreeSet<String>>,
) -> BalancingCredentialSnapshot {
    let capacity = binding.capacity();
    let models = models
        .into_iter()
        .flatten()
        .map(|model| model_health(published, binding, endpoint_id, proxy_id, model))
        .collect();
    BalancingCredentialSnapshot {
        credential_id: binding.credential_id(),
        endpoint_id,
        proxy_id,
        in_flight: capacity.in_flight(),
        max_concurrency: capacity.max_concurrency(),
        fixed_waiters: binding.fixed_waiter_count(),
        auxiliary_in_flight: binding.auxiliary_in_flight(),
        counters: binding.balancing_counters(),
        models,
    }
}

fn model_health(
    published: &PublishedSnapshot,
    binding: &CredentialRuntimeBinding,
    endpoint_id: ProviderEndpointId,
    proxy_id: ProxyProfileId,
    model: &str,
) -> BalancingCredentialModelSnapshot {
    let policy = published.reliability_policy();
    BalancingCredentialModelSnapshot {
        upstream_model: model.to_owned(),
        credential: health_status(binding.generation().health().availability(model)),
        endpoint: published
            .endpoint_health(endpoint_id)
            .map_or(BalancingHealthStatus::Unavailable, |health| {
                health_status(health.availability(&policy))
            }),
        proxy: published
            .proxy_health(proxy_id)
            .map_or(BalancingHealthStatus::Unavailable, |health| {
                health_status(health.availability(&policy))
            }),
    }
}

fn route_models(published: &PublishedSnapshot) -> HashMap<ProviderEndpointId, BTreeSet<String>> {
    let mut models = HashMap::<ProviderEndpointId, BTreeSet<String>>::new();
    for route in published
        .model_routes()
        .routes()
        .iter()
        .filter(|route| route.enabled())
    {
        for target in route.targets().iter().filter(|target| target.enabled()) {
            models
                .entry(target.provider_endpoint_id())
                .or_default()
                .insert(target.upstream_model().as_str().to_owned());
        }
    }
    models
}

fn health_status(result: Result<(), HealthAcquireError>) -> BalancingHealthStatus {
    match result {
        Ok(()) => BalancingHealthStatus::Available,
        Err(HealthAcquireError::Permanent) => BalancingHealthStatus::Unavailable,
        Err(HealthAcquireError::Temporary(until)) => BalancingHealthStatus::Cooling {
            retry_in_ms: duration_ms(until.saturating_duration_since(Instant::now())).max(1),
        },
    }
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
