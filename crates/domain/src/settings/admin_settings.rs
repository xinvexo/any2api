use super::{
    SettingKey, SettingOverrides, SettingsValidationError,
    value::{boolean, integer},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminSettings {
    remote_enabled: bool,
    session_idle_timeout_ms: u64,
    session_absolute_timeout_ms: u64,
    login_failure_window_ms: u64,
    login_max_failures: u64,
}

impl AdminSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        let settings = Self {
            remote_enabled: boolean(value(SettingKey::AdminRemoteEnabled))?,
            session_idle_timeout_ms: integer(value(SettingKey::AdminSessionIdleTimeout))?,
            session_absolute_timeout_ms: integer(value(SettingKey::AdminSessionAbsoluteTimeout))?,
            login_failure_window_ms: integer(value(SettingKey::AdminLoginFailureWindow))?,
            login_max_failures: integer(value(SettingKey::AdminLoginMaxFailures))?,
        };
        if settings.session_idle_timeout_ms > settings.session_absolute_timeout_ms {
            return Err(SettingsValidationError::InvalidCombination);
        }
        Ok(settings)
    }

    pub const fn remote_enabled(&self) -> bool {
        self.remote_enabled
    }

    pub const fn session_idle_timeout_ms(&self) -> u64 {
        self.session_idle_timeout_ms
    }

    pub const fn session_absolute_timeout_ms(&self) -> u64 {
        self.session_absolute_timeout_ms
    }

    pub const fn login_failure_window_ms(&self) -> u64 {
        self.login_failure_window_ms
    }

    pub const fn login_max_failures(&self) -> u64 {
        self.login_max_failures
    }
}
