use super::{
    AffinityMode, SettingKey, SettingOverrides, SettingValue, SettingsValidationError,
    value::{boolean, integer},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinitySettings {
    soft_enabled: bool,
    soft_mode: AffinityMode,
    soft_ttl_ms: u64,
    hard_ttl_ms: u64,
    soft_prefer_wait_timeout_ms: u64,
    fixed_wait_timeout_ms: u64,
}

impl AffinitySettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        let soft_mode = match value(SettingKey::AffinitySoftMode) {
            SettingValue::AffinityMode(value) => value,
            _ => return Err(SettingsValidationError::InvalidType),
        };
        Ok(Self {
            soft_enabled: boolean(value(SettingKey::AffinitySoftEnabled))?,
            soft_mode,
            soft_ttl_ms: integer(value(SettingKey::AffinitySoftTtl))?,
            hard_ttl_ms: integer(value(SettingKey::AffinityHardTtl))?,
            soft_prefer_wait_timeout_ms: integer(value(SettingKey::AffinitySoftPreferWaitTimeout))?,
            fixed_wait_timeout_ms: integer(value(SettingKey::AffinityFixedWaitTimeout))?,
        })
    }

    pub const fn soft_enabled(&self) -> bool {
        self.soft_enabled
    }

    pub const fn soft_mode(&self) -> AffinityMode {
        self.soft_mode
    }

    pub const fn soft_ttl_ms(&self) -> u64 {
        self.soft_ttl_ms
    }

    pub const fn hard_ttl_ms(&self) -> u64 {
        self.hard_ttl_ms
    }

    pub const fn soft_prefer_wait_timeout_ms(&self) -> u64 {
        self.soft_prefer_wait_timeout_ms
    }

    pub const fn fixed_wait_timeout_ms(&self) -> u64 {
        self.fixed_wait_timeout_ms
    }
}
