use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpstreamSettings {
    read_timeout_secs: u64,
    strict_ssrf: bool,
}

impl UpstreamSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        Ok(Self {
            read_timeout_secs: integer(overrides.effective_value(SettingKey::UpstreamReadTimeout))?,
            strict_ssrf: super::value::boolean(
                overrides.effective_value(SettingKey::UpstreamStrictSsrf),
            )?,
        })
    }

    pub const fn read_timeout_secs(&self) -> u64 {
        self.read_timeout_secs
    }

    pub const fn strict_ssrf(&self) -> bool {
        self.strict_ssrf
    }
}
