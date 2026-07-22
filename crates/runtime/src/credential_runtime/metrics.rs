use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CredentialBalancingCounters {
    selected_generation: u64,
    selected_auxiliary: u64,
    filtered_capacity: u64,
    filtered_credential_health: u64,
    filtered_endpoint_health: u64,
    filtered_proxy_health: u64,
}

impl CredentialBalancingCounters {
    pub const fn selected_generation(self) -> u64 {
        self.selected_generation
    }

    pub const fn selected_auxiliary(self) -> u64 {
        self.selected_auxiliary
    }

    pub const fn filtered_capacity(self) -> u64 {
        self.filtered_capacity
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CredentialFilterKind {
    Capacity,
    CredentialHealth,
    EndpointHealth,
    ProxyHealth,
}

#[derive(Debug, Default)]
pub(super) struct CredentialBalancingMetrics {
    selected_generation: AtomicU64,
    selected_auxiliary: AtomicU64,
    filtered_capacity: AtomicU64,
    filtered_credential_health: AtomicU64,
    filtered_endpoint_health: AtomicU64,
    filtered_proxy_health: AtomicU64,
}

impl CredentialBalancingMetrics {
    pub(super) fn record_generation_selection(&self) {
        self.selected_generation.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_auxiliary_selection(&self) {
        self.selected_auxiliary.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn record_filter(&self, kind: CredentialFilterKind) {
        let counter = match kind {
            CredentialFilterKind::Capacity => &self.filtered_capacity,
            CredentialFilterKind::CredentialHealth => &self.filtered_credential_health,
            CredentialFilterKind::EndpointHealth => &self.filtered_endpoint_health,
            CredentialFilterKind::ProxyHealth => &self.filtered_proxy_health,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn snapshot(&self) -> CredentialBalancingCounters {
        CredentialBalancingCounters {
            selected_generation: self.selected_generation.load(Ordering::Relaxed),
            selected_auxiliary: self.selected_auxiliary.load(Ordering::Relaxed),
            filtered_capacity: self.filtered_capacity.load(Ordering::Relaxed),
            filtered_credential_health: self.filtered_credential_health.load(Ordering::Relaxed),
            filtered_endpoint_health: self.filtered_endpoint_health.load(Ordering::Relaxed),
            filtered_proxy_health: self.filtered_proxy_health.load(Ordering::Relaxed),
        }
    }
}
