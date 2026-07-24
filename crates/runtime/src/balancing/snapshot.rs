use any2api_domain::{ProviderEndpointId, ProxyProfileId};
use tokio::time::Instant;

use super::{
    BalancingAuxiliarySnapshot, BalancingCredentialModelSnapshot, BalancingCredentialSnapshot,
    BalancingHealthStatus, BalancingQueueSnapshot, BalancingRuntimeSnapshot,
};
use crate::{
    credential_runtime::CredentialRuntimeBinding, health::HealthAcquireError,
    published_snapshot::PublishedSnapshot, queue::SaturationAction, registry::RuntimeRegistry,
    routing_credential::RoutingCredential,
};

pub(crate) fn snapshot(
    runtime: &RuntimeRegistry,
    published: &PublishedSnapshot,
) -> BalancingRuntimeSnapshot {
    let queue_policy = published.queue_policy();
    let (auxiliary_in_flight, auxiliary_limits) =
        published.auxiliary_scheduler().runtime_capacity();
    let credentials = published
        .routing_credentials()
        .iter()
        .filter_map(|credential| {
            published.proxies().get(credential.proxy_id())?;
            Some(credential_snapshot(published, credential))
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
    credential: &RoutingCredential,
) -> BalancingCredentialSnapshot {
    let binding = credential.binding();
    let capacity = binding.capacity();
    let models = credential
        .models()
        .iter()
        .map(|model| {
            model_health(
                published,
                binding,
                credential.endpoint_id(),
                credential.proxy_id(),
                model.as_str(),
            )
        })
        .collect();
    BalancingCredentialSnapshot {
        credential_id: binding.credential_id(),
        label: credential.label().to_owned(),
        provider_kind: credential.provider_kind(),
        enabled: credential.enabled(),
        authentication_expired: credential.authentication_expired(),
        provider_endpoint_id: credential
            .id()
            .provider_credential_id()
            .map(|_| credential.endpoint_id()),
        endpoint_name: credential
            .id()
            .provider_credential_id()
            .map(|_| credential.endpoint_name().to_owned()),
        endpoint_enabled: credential.endpoint_enabled(),
        proxy_id: credential.proxy_id(),
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
