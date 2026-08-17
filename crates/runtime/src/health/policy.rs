use std::time::Duration;

use any2api_domain::ReliabilitySettings;

const MAX_TOTAL_ATTEMPTS: u32 = 3;
const MAX_CREDENTIAL_SWITCHES: u32 = 2;
const MAX_SAME_CREDENTIAL_RETRIES: u32 = 1;
const BASE_DELAY: Duration = Duration::from_secs(1);
const MAX_DELAY: Duration = Duration::from_secs(2);
const JITTER_RATIO: u32 = 20;
const RATE_LIMIT_FALLBACK: Duration = Duration::from_secs(60);
const MODEL_UNSUPPORTED: Duration = Duration::from_secs(3_600);
const PERMISSION_DENIED: Duration = Duration::from_secs(900);
const TRANSIENT_ENDPOINT: Duration = Duration::from_secs(15);
const ENDPOINT_FAILURE_THRESHOLD: u32 = 3;
const ENDPOINT_FAILURE_WINDOW: Duration = Duration::from_secs(30);
const ENDPOINT_OPEN_DURATION: Duration = Duration::from_secs(15);
const PROXY_FAILURE_THRESHOLD: u32 = 3;
const PROXY_FAILURE_WINDOW: Duration = Duration::from_secs(30);
const PROXY_OPEN_DURATION: Duration = Duration::from_secs(30);
const HALF_OPEN_MAX_PROBES: u32 = 1;

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
            max_total_attempts: MAX_TOTAL_ATTEMPTS,
            max_credential_switches: MAX_CREDENTIAL_SWITCHES,
            max_same_credential_retries: MAX_SAME_CREDENTIAL_RETRIES,
            precommit_total_budget: Duration::from_secs(settings.precommit_total_budget_secs()),
            base_delay: BASE_DELAY,
            max_delay: MAX_DELAY,
            jitter_ratio: JITTER_RATIO,
            rate_limit_fallback: RATE_LIMIT_FALLBACK,
            model_unsupported: MODEL_UNSUPPORTED,
            permission_denied: PERMISSION_DENIED,
            transient_endpoint: TRANSIENT_ENDPOINT,
            endpoint_failure_threshold: ENDPOINT_FAILURE_THRESHOLD,
            endpoint_failure_window: ENDPOINT_FAILURE_WINDOW,
            endpoint_open_duration: ENDPOINT_OPEN_DURATION,
            proxy_failure_threshold: PROXY_FAILURE_THRESHOLD,
            proxy_failure_window: PROXY_FAILURE_WINDOW,
            proxy_open_duration: PROXY_OPEN_DURATION,
            half_open_max_probes: HALF_OPEN_MAX_PROBES,
        }
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{SettingKey, SettingOverrides, SettingValue, SettingsConfiguration};

    use super::*;

    #[test]
    fn only_the_total_budget_is_configurable() {
        let settings = SettingsConfiguration::from_overrides(
            SettingOverrides::from_entries([(
                SettingKey::RetryPrecommitTotalBudget,
                SettingValue::DurationSecs(42),
            )])
            .expect("valid override"),
        )
        .expect("settings");

        assert_eq!(
            ReliabilityPolicy::from_settings(settings.reliability()),
            ReliabilityPolicy {
                max_total_attempts: 3,
                max_credential_switches: 2,
                max_same_credential_retries: 1,
                precommit_total_budget: Duration::from_secs(42),
                base_delay: Duration::from_secs(1),
                max_delay: Duration::from_secs(2),
                jitter_ratio: 20,
                rate_limit_fallback: Duration::from_secs(60),
                model_unsupported: Duration::from_secs(3_600),
                permission_denied: Duration::from_secs(900),
                transient_endpoint: Duration::from_secs(15),
                endpoint_failure_threshold: 3,
                endpoint_failure_window: Duration::from_secs(30),
                endpoint_open_duration: Duration::from_secs(15),
                proxy_failure_threshold: 3,
                proxy_failure_window: Duration::from_secs(30),
                proxy_open_duration: Duration::from_secs(30),
                half_open_max_probes: 1,
            }
        );
    }
}
