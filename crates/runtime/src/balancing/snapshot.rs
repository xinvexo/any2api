use any2api_domain::{ProviderEndpointId, ProxyProfileId};
use tokio::time::Instant;

use super::{
    BalancingCredentialModelSnapshot, BalancingCredentialSnapshot, BalancingHealthStatus,
    BalancingQueueSnapshot, BalancingRuntimeSnapshot,
};
use crate::{
    credential_runtime::CredentialRuntimeBinding, health::HealthAcquireError,
    published_snapshot::PublishedSnapshot, queue::RateLimitAction, registry::RuntimeRegistry,
    routing_credential::RoutingCredential,
};

pub(crate) fn snapshot(
    runtime: &RuntimeRegistry,
    published: &PublishedSnapshot,
) -> BalancingRuntimeSnapshot {
    let queue_policy = published.queue_policy();
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
            rejects_when_rate_limited: queue_policy.on_rate_limited() == RateLimitAction::Reject,
            fallback_on_rate_limit: queue_policy.fallback_on_rate_limit(),
        },
        credentials,
    }
}

fn credential_snapshot(
    published: &PublishedSnapshot,
    credential: &RoutingCredential,
) -> BalancingCredentialSnapshot {
    let binding = credential.binding();
    let rate = binding.rate_snapshot();
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
        in_flight: binding.in_flight(),
        requests_per_minute: rate.requests_per_minute(),
        requests_in_window: rate.requests_in_window(),
        remaining_requests: rate.remaining(),
        retry_in_ms: rate
            .retry_at()
            .map(|retry_at| duration_ms(retry_at.saturating_duration_since(Instant::now())).max(1)),
        fixed_waiters: binding.fixed_waiter_count(),
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
