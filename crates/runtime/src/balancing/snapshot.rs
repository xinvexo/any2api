use std::collections::BTreeMap;

use any2api_domain::ProviderKind;

use super::{
    BalancingProviderSnapshot, BalancingQueueSnapshot, BalancingRuntimeSnapshot,
    BalancingTotalsSnapshot,
};
use crate::{
    published_snapshot::PublishedSnapshot, queue::RateLimitAction, registry::RuntimeRegistry,
    routing_credential::RoutingCredential,
};

pub(crate) fn snapshot(
    runtime: &RuntimeRegistry,
    published: &PublishedSnapshot,
) -> BalancingRuntimeSnapshot {
    let queue_policy = published.queue_policy();
    let mut aggregate = BalancingAggregate::default();
    for credential in published.routing_credentials() {
        let Some(proxy) = published.proxies().get(credential.proxy_id()) else {
            continue;
        };
        aggregate.record(CredentialAggregate::new(credential, proxy.enabled()));
    }

    BalancingRuntimeSnapshot {
        scheduler_epoch: runtime.scheduler_epoch(),
        queue: BalancingQueueSnapshot {
            waiting: runtime.queue_waiting_count(),
            max_waiting: queue_policy.max_waiting_requests(),
            timeout_secs: queue_policy.queue_timeout().as_secs(),
            rejects_when_rate_limited: queue_policy.on_rate_limited() == RateLimitAction::Reject,
            fallback_on_rate_limit: queue_policy.fallback_on_rate_limit(),
        },
        totals: aggregate.totals,
        providers: aggregate.providers.into_values().collect(),
    }
}

#[derive(Debug, Default)]
struct BalancingAggregate {
    totals: BalancingTotalsSnapshot,
    providers: BTreeMap<ProviderKind, BalancingProviderSnapshot>,
}

impl BalancingAggregate {
    fn record(&mut self, credential: CredentialAggregate) {
        add_credential(&mut self.totals, credential);
        let provider =
            self.providers
                .entry(credential.provider_kind)
                .or_insert(BalancingProviderSnapshot {
                    provider_kind: credential.provider_kind,
                    credential_count: 0,
                    enabled_credential_count: 0,
                    limited_credential_count: 0,
                    rate_limited_credential_count: 0,
                    in_flight: 0,
                    requests_in_window: 0,
                    fixed_waiters: 0,
                    selected: 0,
                });
        add_credential(provider, credential);
    }
}

#[derive(Clone, Copy, Debug)]
struct CredentialAggregate {
    provider_kind: ProviderKind,
    enabled: bool,
    limited: bool,
    rate_limited: bool,
    in_flight: u64,
    requests_in_window: u64,
    fixed_waiters: u64,
    selected: u64,
}

impl CredentialAggregate {
    fn new(credential: &RoutingCredential, proxy_enabled: bool) -> Self {
        let binding = credential.binding();
        let rate = binding.rate_snapshot();
        Self {
            provider_kind: credential.provider_kind(),
            enabled: credential.enabled()
                && !credential.authentication_expired()
                && credential.endpoint_enabled()
                && proxy_enabled,
            limited: rate.requests_per_minute().is_some(),
            rate_limited: matches!(rate.remaining(), Some(0)),
            in_flight: u64::from(binding.in_flight()),
            requests_in_window: u64::from(rate.requests_in_window()),
            fixed_waiters: u64::from(binding.fixed_waiter_count()),
            selected: binding.balancing_counters().selected(),
        }
    }
}

trait AggregateTarget {
    fn add(&mut self, credential: CredentialAggregate);
}

impl AggregateTarget for BalancingTotalsSnapshot {
    fn add(&mut self, credential: CredentialAggregate) {
        self.credential_count += 1;
        self.enabled_credential_count += usize::from(credential.enabled);
        self.limited_credential_count += usize::from(credential.limited);
        self.rate_limited_credential_count += usize::from(credential.rate_limited);
        self.in_flight += credential.in_flight;
        self.requests_in_window += credential.requests_in_window;
        self.fixed_waiters += credential.fixed_waiters;
        self.selected += credential.selected;
    }
}

impl AggregateTarget for BalancingProviderSnapshot {
    fn add(&mut self, credential: CredentialAggregate) {
        self.credential_count += 1;
        self.enabled_credential_count += usize::from(credential.enabled);
        self.limited_credential_count += usize::from(credential.limited);
        self.rate_limited_credential_count += usize::from(credential.rate_limited);
        self.in_flight += credential.in_flight;
        self.requests_in_window += credential.requests_in_window;
        self.fixed_waiters += credential.fixed_waiters;
        self.selected += credential.selected;
    }
}

fn add_credential(target: &mut impl AggregateTarget, credential: CredentialAggregate) {
    target.add(credential);
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProviderKind;

    use super::{BalancingAggregate, CredentialAggregate};

    #[test]
    fn aggregation_keeps_only_global_and_provider_totals() {
        let mut aggregate = BalancingAggregate::default();
        aggregate.record(credential(ProviderKind::Codex, true, true, false, 2, 7));
        aggregate.record(credential(ProviderKind::Codex, false, true, true, 0, 3));
        aggregate.record(credential(ProviderKind::Claude, true, false, false, 1, 0));

        assert_eq!(aggregate.totals.credential_count(), 3);
        assert_eq!(aggregate.totals.enabled_credential_count(), 2);
        assert_eq!(aggregate.totals.limited_credential_count(), 2);
        assert_eq!(aggregate.totals.rate_limited_credential_count(), 1);
        assert_eq!(aggregate.totals.in_flight(), 3);
        assert_eq!(aggregate.totals.requests_in_window(), 10);
        assert_eq!(aggregate.totals.fixed_waiters(), 3);
        assert_eq!(aggregate.totals.selected(), 22);

        let codex = aggregate.providers[&ProviderKind::Codex];
        assert_eq!(codex.credential_count(), 2);
        assert_eq!(codex.enabled_credential_count(), 1);
        assert_eq!(codex.rate_limited_credential_count(), 1);
        assert_eq!(codex.requests_in_window(), 10);
        assert_eq!(codex.selected(), 18);
    }

    fn credential(
        provider_kind: ProviderKind,
        enabled: bool,
        limited: bool,
        rate_limited: bool,
        in_flight: u64,
        requests_in_window: u64,
    ) -> CredentialAggregate {
        CredentialAggregate {
            provider_kind,
            enabled,
            limited,
            rate_limited,
            in_flight,
            requests_in_window,
            fixed_waiters: in_flight,
            selected: requests_in_window + 4,
        }
    }
}
