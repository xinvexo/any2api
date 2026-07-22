use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShutdownSettings {
    request_grace_period_ms: u64,
    finalize_timeout_ms: u64,
}

impl ShutdownSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        Ok(Self {
            request_grace_period_ms: integer(
                overrides.effective_value(SettingKey::ShutdownRequestGracePeriod),
            )?,
            finalize_timeout_ms: integer(
                overrides.effective_value(SettingKey::ShutdownFinalizeTimeout),
            )?,
        })
    }

    pub const fn request_grace_period_ms(&self) -> u64 {
        self.request_grace_period_ms
    }

    pub const fn finalize_timeout_ms(&self) -> u64 {
        self.finalize_timeout_ms
    }
}
