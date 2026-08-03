use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CredentialBalancingCounters {
    selected: u64,
    filtered_rate_limit: u64,
    filtered_credential_health: u64,
    filtered_endpoint_health: u64,
    filtered_proxy_health: u64,
}

impl CredentialBalancingCounters {
    pub const fn selected(self) -> u64 {
        self.selected
    }

    pub const fn filtered_rate_limit(self) -> u64 {
        self.filtered_rate_limit
    }

    pub const fn filtered_credential_health(self) -> u64 {
        self.filtered_credential_health
    }

    pub const fn filtered_endpoint_health(self) -> u64 {
        self.filtered_endpoint_health
    }

    pub const fn filtered_proxy_health(self) -> u64 {
        self.filtered_proxy_health
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum CredentialFilterKind {
    RateLimit,
    CredentialHealth,
    EndpointHealth,
    ProxyHealth,
}

#[derive(Debug, Default)]
pub(super) struct CredentialBalancingMetrics {
    selected: AtomicU64,
    filtered_rate_limit: AtomicU64,
    filtered_credential_health: AtomicU64,
    filtered_endpoint_health: AtomicU64,
    filtered_proxy_health: AtomicU64,
}

impl CredentialBalancingMetrics {
    pub(super) fn record_selection(&self) {
        self.selected.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_filter(&self, kind: CredentialFilterKind) {
        let counter = match kind {
            CredentialFilterKind::RateLimit => &self.filtered_rate_limit,
            CredentialFilterKind::CredentialHealth => &self.filtered_credential_health,
            CredentialFilterKind::EndpointHealth => &self.filtered_endpoint_health,
            CredentialFilterKind::ProxyHealth => &self.filtered_proxy_health,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn snapshot(&self) -> CredentialBalancingCounters {
        CredentialBalancingCounters {
            selected: self.selected.load(Ordering::Relaxed),
            filtered_rate_limit: self.filtered_rate_limit.load(Ordering::Relaxed),
            filtered_credential_health: self.filtered_credential_health.load(Ordering::Relaxed),
            filtered_endpoint_health: self.filtered_endpoint_health.load(Ordering::Relaxed),
            filtered_proxy_health: self.filtered_proxy_health.load(Ordering::Relaxed),
        }
    }
}
