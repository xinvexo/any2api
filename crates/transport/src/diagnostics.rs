use std::time::Duration;

use any2api_domain::ProxyKind;

use crate::{
    api::{TransportManagerConfig, TransportProxy, TransportRequest},
    isolation::TransportTrafficClass,
    profile::GENERIC_GATEWAY_TRANSPORT_PROFILE as WIRE_PROFILE,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportResolverMode {
    System,
    ProxyRemote,
    LocalCached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportRequestDiagnostics {
    wire_profile_id: &'static str,
    wire_profile_version: u16,
    timeout_policy_version: u16,
    resolver_mode: TransportResolverMode,
    proxy_kind: ProxyKind,
    connect_timeout: Duration,
    read_timeout: Duration,
    pool_idle_timeout: Duration,
    routing_generation: u64,
    authentication_version: u64,
    traffic_class: TransportTrafficClass,
}

impl TransportRequestDiagnostics {
    #[must_use]
    pub const fn wire_profile_id(self) -> &'static str {
        self.wire_profile_id
    }

    #[must_use]
    pub const fn wire_profile_version(self) -> u16 {
        self.wire_profile_version
    }

    #[must_use]
    pub const fn timeout_policy_version(self) -> u16 {
        self.timeout_policy_version
    }

    #[must_use]
    pub const fn resolver_mode(self) -> TransportResolverMode {
        self.resolver_mode
    }

    #[must_use]
    pub const fn proxy_kind(self) -> ProxyKind {
        self.proxy_kind
    }

    #[must_use]
    pub const fn connect_timeout(self) -> Duration {
        self.connect_timeout
    }

    #[must_use]
    pub const fn read_timeout(self) -> Duration {
        self.read_timeout
    }

    #[must_use]
    pub const fn pool_idle_timeout(self) -> Duration {
        self.pool_idle_timeout
    }

    #[must_use]
    pub const fn routing_generation(self) -> u64 {
        self.routing_generation
    }

    #[must_use]
    pub const fn authentication_version(self) -> u64 {
        self.authentication_version
    }

    #[must_use]
    pub const fn traffic_class(self) -> TransportTrafficClass {
        self.traffic_class
    }
}

pub(crate) fn for_request(
    config: TransportManagerConfig,
    proxy: TransportProxy<'_>,
    request: &TransportRequest,
) -> TransportRequestDiagnostics {
    let proxy_kind = proxy.profile().kind();
    let resolver_mode = if request.network_policy.strict_ssrf() {
        TransportResolverMode::LocalCached
    } else if proxy_kind == ProxyKind::Direct {
        TransportResolverMode::System
    } else {
        TransportResolverMode::ProxyRemote
    };
    TransportRequestDiagnostics {
        wire_profile_id: WIRE_PROFILE.id(),
        wire_profile_version: WIRE_PROFILE.policy_version(),
        timeout_policy_version: WIRE_PROFILE.timeout_policy_version(),
        resolver_mode,
        proxy_kind,
        connect_timeout: config.connect_timeout,
        read_timeout: request.read_timeout,
        pool_idle_timeout: config.pool_idle_timeout,
        routing_generation: request.isolation.routing_generation(),
        authentication_version: request.isolation.authentication_version(),
        traffic_class: request.isolation.traffic_class(),
    }
}
