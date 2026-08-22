use any2api_domain::{ProxyProfile, RoutingCredentialId};

use super::PublishedSnapshot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialRuntimeStatus {
    Ready,
    Disabled,
    EndpointDisabled,
    AuthenticationExpired,
    RateLimited,
    ProxyDisabled,
}

impl CredentialRuntimeStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Disabled => "disabled",
            Self::EndpointDisabled => "endpoint_disabled",
            Self::AuthenticationExpired => "authentication_expired",
            Self::RateLimited => "rate_limited",
            Self::ProxyDisabled => "proxy_disabled",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CredentialRuntimeObservation<'a> {
    resolved_proxy: &'a ProxyProfile,
    rpm_window_used: u32,
    rpm_limit: Option<u32>,
    in_flight: u32,
    status: CredentialRuntimeStatus,
}

impl<'a> CredentialRuntimeObservation<'a> {
    #[must_use]
    pub const fn resolved_proxy(self) -> &'a ProxyProfile {
        self.resolved_proxy
    }

    #[must_use]
    pub const fn rpm_window_used(self) -> u32 {
        self.rpm_window_used
    }

    #[must_use]
    pub const fn rpm_limit(self) -> Option<u32> {
        self.rpm_limit
    }

    #[must_use]
    pub const fn in_flight(self) -> u32 {
        self.in_flight
    }

    #[must_use]
    pub const fn status(self) -> CredentialRuntimeStatus {
        self.status
    }
}

impl PublishedSnapshot {
    #[must_use]
    pub fn credential_runtime_observation(
        &self,
        id: RoutingCredentialId,
    ) -> Option<CredentialRuntimeObservation<'_>> {
        let credential = self.routing_credentials.get(id)?;
        let resolved_proxy = self.proxies().get(credential.proxy_id())?;
        let binding = credential.binding();
        let rate = binding.rate_snapshot();
        let status = runtime_status(
            credential.enabled(),
            credential.endpoint_enabled(),
            credential.authentication_expired(),
            resolved_proxy.enabled(),
            matches!(rate.remaining(), Some(0)),
        );
        Some(CredentialRuntimeObservation {
            resolved_proxy,
            rpm_window_used: rate.requests_in_window(),
            rpm_limit: rate.requests_per_minute(),
            in_flight: binding.in_flight(),
            status,
        })
    }
}

const fn runtime_status(
    enabled: bool,
    endpoint_enabled: bool,
    authentication_expired: bool,
    proxy_enabled: bool,
    rate_limited: bool,
) -> CredentialRuntimeStatus {
    if !enabled {
        CredentialRuntimeStatus::Disabled
    } else if !endpoint_enabled {
        CredentialRuntimeStatus::EndpointDisabled
    } else if authentication_expired {
        CredentialRuntimeStatus::AuthenticationExpired
    } else if !proxy_enabled {
        CredentialRuntimeStatus::ProxyDisabled
    } else if rate_limited {
        CredentialRuntimeStatus::RateLimited
    } else {
        CredentialRuntimeStatus::Ready
    }
}

#[cfg(test)]
mod tests {
    use super::{CredentialRuntimeStatus, runtime_status};

    #[test]
    fn finite_status_has_one_deterministic_precedence() {
        assert_eq!(
            runtime_status(false, false, true, false, true),
            CredentialRuntimeStatus::Disabled
        );
        assert_eq!(
            runtime_status(true, false, true, false, true),
            CredentialRuntimeStatus::EndpointDisabled
        );
        assert_eq!(
            runtime_status(true, true, true, false, true),
            CredentialRuntimeStatus::AuthenticationExpired
        );
        assert_eq!(
            runtime_status(true, true, false, false, true),
            CredentialRuntimeStatus::ProxyDisabled
        );
        assert_eq!(
            runtime_status(true, true, false, true, true),
            CredentialRuntimeStatus::RateLimited
        );
        assert_eq!(
            runtime_status(true, true, false, true, false),
            CredentialRuntimeStatus::Ready
        );
    }
}
