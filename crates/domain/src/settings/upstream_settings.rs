use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamSettings {
    read_timeout_ms: u64,
}

impl UpstreamSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        Ok(Self {
            read_timeout_ms: integer(overrides.effective_value(SettingKey::UpstreamReadTimeout))?,
        })
    }

    pub const fn read_timeout_ms(&self) -> u64 {
        self.read_timeout_ms
    }
}
