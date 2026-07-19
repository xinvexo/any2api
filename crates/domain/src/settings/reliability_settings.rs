use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReliabilitySettings {
    max_total_attempts: u64,
    max_credential_switches: u64,
    max_same_credential_retries: u64,
    precommit_total_budget_ms: u64,
    base_delay_ms: u64,
    max_delay_ms: u64,
    jitter_ratio: u64,
    rate_limit_fallback_ms: u64,
    model_unsupported_ms: u64,
    permission_denied_ms: u64,
    transient_endpoint_ms: u64,
    endpoint_failure_threshold: u64,
    endpoint_failure_window_ms: u64,
    endpoint_open_duration_ms: u64,
    proxy_failure_threshold: u64,
    proxy_failure_window_ms: u64,
    proxy_open_duration_ms: u64,
    half_open_max_probes: u64,
}

impl ReliabilitySettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| integer(overrides.effective_value(key));
        let settings = Self {
            max_total_attempts: value(SettingKey::RetryMaxTotalAttempts)?,
            max_credential_switches: value(SettingKey::RetryMaxCredentialSwitches)?,
            max_same_credential_retries: value(SettingKey::RetryMaxSameCredentialRetries)?,
            precommit_total_budget_ms: value(SettingKey::RetryPrecommitTotalBudget)?,
            base_delay_ms: value(SettingKey::RetryBaseDelay)?,
            max_delay_ms: value(SettingKey::RetryMaxDelay)?,
            jitter_ratio: value(SettingKey::RetryJitterRatio)?,
            rate_limit_fallback_ms: value(SettingKey::CooldownRateLimitFallback)?,
            model_unsupported_ms: value(SettingKey::CooldownModelUnsupported)?,
            permission_denied_ms: value(SettingKey::CooldownPermissionDenied)?,
            transient_endpoint_ms: value(SettingKey::CooldownTransientEndpoint)?,
            endpoint_failure_threshold: value(SettingKey::BreakerEndpointFailureThreshold)?,
            endpoint_failure_window_ms: value(SettingKey::BreakerEndpointFailureWindow)?,
            endpoint_open_duration_ms: value(SettingKey::BreakerEndpointOpenDuration)?,
            proxy_failure_threshold: value(SettingKey::BreakerProxyFailureThreshold)?,
            proxy_failure_window_ms: value(SettingKey::BreakerProxyFailureWindow)?,
            proxy_open_duration_ms: value(SettingKey::BreakerProxyOpenDuration)?,
            half_open_max_probes: value(SettingKey::BreakerHalfOpenMaxProbes)?,
        };
        if settings.max_delay_ms < settings.base_delay_ms {
            return Err(SettingsValidationError::InvalidCombination);
        }
        Ok(settings)
    }

    pub const fn max_total_attempts(&self) -> u64 {
        self.max_total_attempts
    }
    pub const fn max_credential_switches(&self) -> u64 {
        self.max_credential_switches
    }
    pub const fn max_same_credential_retries(&self) -> u64 {
        self.max_same_credential_retries
    }
    pub const fn precommit_total_budget_ms(&self) -> u64 {
        self.precommit_total_budget_ms
    }
    pub const fn base_delay_ms(&self) -> u64 {
        self.base_delay_ms
    }
    pub const fn max_delay_ms(&self) -> u64 {
        self.max_delay_ms
    }
    pub const fn jitter_ratio(&self) -> u64 {
        self.jitter_ratio
    }
    pub const fn rate_limit_fallback_ms(&self) -> u64 {
        self.rate_limit_fallback_ms
    }
    pub const fn model_unsupported_ms(&self) -> u64 {
        self.model_unsupported_ms
    }
    pub const fn permission_denied_ms(&self) -> u64 {
        self.permission_denied_ms
    }
    pub const fn transient_endpoint_ms(&self) -> u64 {
        self.transient_endpoint_ms
    }
    pub const fn endpoint_failure_threshold(&self) -> u64 {
        self.endpoint_failure_threshold
    }
    pub const fn endpoint_failure_window_ms(&self) -> u64 {
        self.endpoint_failure_window_ms
    }
    pub const fn endpoint_open_duration_ms(&self) -> u64 {
        self.endpoint_open_duration_ms
    }
    pub const fn proxy_failure_threshold(&self) -> u64 {
        self.proxy_failure_threshold
    }
    pub const fn proxy_failure_window_ms(&self) -> u64 {
        self.proxy_failure_window_ms
    }
    pub const fn proxy_open_duration_ms(&self) -> u64 {
        self.proxy_open_duration_ms
    }
    pub const fn half_open_max_probes(&self) -> u64 {
        self.half_open_max_probes
    }
}
