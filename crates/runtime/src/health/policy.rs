use std::time::Duration;

use any2api_domain::ReliabilitySettings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReliabilityPolicy {
    pub(crate) max_total_attempts: u32,
    pub(crate) max_credential_switches: u32,
    pub(crate) max_same_credential_retries: u32,
    pub(crate) precommit_total_budget: Duration,
    pub(crate) base_delay: Duration,
    pub(crate) max_delay: Duration,
    pub(crate) jitter_ratio: u32,
    pub(crate) rate_limit_fallback: Duration,
    pub(crate) model_unsupported: Duration,
    pub(crate) permission_denied: Duration,
    pub(crate) transient_endpoint: Duration,
    pub(crate) endpoint_failure_threshold: u32,
    pub(crate) endpoint_failure_window: Duration,
    pub(crate) endpoint_open_duration: Duration,
    pub(crate) proxy_failure_threshold: u32,
    pub(crate) proxy_failure_window: Duration,
    pub(crate) proxy_open_duration: Duration,
    pub(crate) half_open_max_probes: u32,
}

impl ReliabilityPolicy {
    pub(crate) fn from_settings(settings: &ReliabilitySettings) -> Self {
        Self {
            max_total_attempts: settings.max_total_attempts() as u32,
            max_credential_switches: settings.max_credential_switches() as u32,
            max_same_credential_retries: settings.max_same_credential_retries() as u32,
            precommit_total_budget: Duration::from_secs(settings.precommit_total_budget_secs()),
            base_delay: Duration::from_secs(settings.base_delay_secs()),
            max_delay: Duration::from_secs(settings.max_delay_secs()),
            jitter_ratio: settings.jitter_ratio() as u32,
            rate_limit_fallback: Duration::from_secs(settings.rate_limit_fallback_secs()),
            model_unsupported: Duration::from_secs(settings.model_unsupported_secs()),
            permission_denied: Duration::from_secs(settings.permission_denied_secs()),
            transient_endpoint: Duration::from_secs(settings.transient_endpoint_secs()),
            endpoint_failure_threshold: settings.endpoint_failure_threshold() as u32,
            endpoint_failure_window: Duration::from_secs(settings.endpoint_failure_window_secs()),
            endpoint_open_duration: Duration::from_secs(settings.endpoint_open_duration_secs()),
            proxy_failure_threshold: settings.proxy_failure_threshold() as u32,
            proxy_failure_window: Duration::from_secs(settings.proxy_failure_window_secs()),
            proxy_open_duration: Duration::from_secs(settings.proxy_open_duration_secs()),
            half_open_max_probes: settings.half_open_max_probes() as u32,
        }
    }
}
