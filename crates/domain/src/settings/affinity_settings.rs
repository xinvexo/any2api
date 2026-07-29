use super::{
    SettingKey, SettingOverrides, SettingsValidationError,
    value::{boolean, integer},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinitySettings {
    enabled: bool,
    ttl_secs: u64,
    wait_timeout_secs: u64,
}

impl AffinitySettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        Ok(Self {
            enabled: boolean(value(SettingKey::AffinityEnabled))?,
            ttl_secs: integer(value(SettingKey::AffinityTtl))?,
            wait_timeout_secs: integer(value(SettingKey::AffinityWaitTimeout))?,
        })
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn ttl_secs(&self) -> u64 {
        self.ttl_secs
    }

    pub const fn wait_timeout_secs(&self) -> u64 {
        self.wait_timeout_secs
    }
}
