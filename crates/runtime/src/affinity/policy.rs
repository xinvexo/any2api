use std::time::Duration;

use any2api_domain::AffinitySettings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffinityPolicy {
    enabled: bool,
    ttl: Duration,
    wait_timeout: Duration,
}

impl AffinityPolicy {
    pub(crate) fn from_settings(settings: &AffinitySettings) -> Self {
        Self {
            enabled: settings.enabled(),
            ttl: Duration::from_secs(settings.ttl_secs()),
            wait_timeout: Duration::from_secs(settings.wait_timeout_secs()),
        }
    }

    pub const fn enabled(self) -> bool {
        self.enabled
    }

    pub const fn ttl(self) -> Duration {
        self.ttl
    }

    pub const fn wait_timeout(self) -> Duration {
        self.wait_timeout
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{SettingKey, SettingOverrides, SettingValue, SettingsConfiguration};

    use super::AffinityPolicy;

    #[test]
    fn policy_captures_the_explicit_session_toggle() {
        let defaults = SettingsConfiguration::defaults();
        assert!(!AffinityPolicy::from_settings(defaults.affinity()).enabled());

        let overrides = SettingOverrides::from_entries([(
            SettingKey::AffinityEnabled,
            SettingValue::Boolean(true),
        )])
        .expect("affinity override");
        let settings = SettingsConfiguration::from_overrides(overrides).expect("settings");
        assert!(AffinityPolicy::from_settings(settings.affinity()).enabled());
    }
}
