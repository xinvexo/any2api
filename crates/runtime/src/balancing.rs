use crate::credential_runtime::CredentialBalancingCounters;
use any2api_domain::{
    CredentialId, OAuthAccountId, ProviderEndpointId, ProviderKind, ProxyProfileId,
    RoutingCredentialId,
};

mod snapshot;

pub(crate) use snapshot::snapshot;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalancingRuntimeSnapshot {
    scheduler_epoch: u64,
    queue: BalancingQueueSnapshot,
    credentials: Vec<BalancingCredentialSnapshot>,
}

impl BalancingRuntimeSnapshot {
    pub const fn scheduler_epoch(&self) -> u64 {
        self.scheduler_epoch
    }

    pub const fn queue(&self) -> BalancingQueueSnapshot {
        self.queue
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
    rejects_when_rate_limited: bool,
    fallback_on_rate_limit: bool,
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

    pub const fn rejects_when_rate_limited(self) -> bool {
        self.rejects_when_rate_limited
    }

    pub const fn fallback_on_rate_limit(self) -> bool {
        self.fallback_on_rate_limit
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BalancingCredentialSnapshot {
    credential_id: RoutingCredentialId,
    label: String,
    provider_kind: ProviderKind,
    enabled: bool,
    authentication_expired: bool,
    provider_endpoint_id: Option<ProviderEndpointId>,
    endpoint_name: Option<String>,
    endpoint_enabled: bool,
    proxy_id: ProxyProfileId,
    in_flight: u32,
    requests_per_minute: Option<u32>,
    requests_in_window: u32,
    remaining_requests: Option<u32>,
    retry_in_ms: Option<u64>,
    fixed_waiters: u32,
    counters: CredentialBalancingCounters,
    models: Vec<BalancingCredentialModelSnapshot>,
}

impl BalancingCredentialSnapshot {
    pub const fn credential_id(&self) -> RoutingCredentialId {
        self.credential_id
    }

    pub const fn provider_credential_id(&self) -> Option<CredentialId> {
        self.credential_id.provider_credential_id()
    }

    pub const fn oauth_account_id(&self) -> Option<OAuthAccountId> {
        self.credential_id.oauth_account_id()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn authentication_expired(&self) -> bool {
        self.authentication_expired
    }

    pub const fn provider_endpoint_id(&self) -> Option<ProviderEndpointId> {
        self.provider_endpoint_id
    }

    pub fn endpoint_name(&self) -> Option<&str> {
        self.endpoint_name.as_deref()
    }

    pub const fn endpoint_enabled(&self) -> bool {
        self.endpoint_enabled
    }

    pub const fn proxy_id(&self) -> ProxyProfileId {
        self.proxy_id
    }

    pub const fn in_flight(&self) -> u32 {
        self.in_flight
    }

    pub const fn requests_per_minute(&self) -> Option<u32> {
        self.requests_per_minute
    }

    pub const fn requests_in_window(&self) -> u32 {
        self.requests_in_window
    }

    pub const fn remaining_requests(&self) -> Option<u32> {
        self.remaining_requests
    }

    pub const fn retry_in_ms(&self) -> Option<u64> {
        self.retry_in_ms
    }

    pub const fn fixed_waiters(&self) -> u32 {
        self.fixed_waiters
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
