use super::{
    RateLimitMode, SettingKey, SettingOverrides, SettingValue, SettingsValidationError,
    value::integer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerSettings {
    on_rate_limited: RateLimitMode,
    queue_timeout_secs: u64,
    max_waiting_requests: u64,
    fallback_on_rate_limit: bool,
}

impl SchedulerSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        let on_rate_limited = match value(SettingKey::SchedulerOnRateLimited) {
            SettingValue::RateLimitMode(value) => value,
            _ => return Err(SettingsValidationError::InvalidType),
        };
        Ok(Self {
            on_rate_limited,
            queue_timeout_secs: integer(value(SettingKey::SchedulerQueueTimeout))?,
            max_waiting_requests: integer(value(SettingKey::SchedulerMaxWaitingRequests))?,
            fallback_on_rate_limit: match value(SettingKey::SchedulerFallbackOnRateLimit) {
                SettingValue::Boolean(value) => value,
                _ => return Err(SettingsValidationError::InvalidType),
            },
        })
    }

    pub const fn on_rate_limited(&self) -> RateLimitMode {
        self.on_rate_limited
    }

    pub const fn queue_timeout_secs(&self) -> u64 {
        self.queue_timeout_secs
    }

    pub const fn max_waiting_requests(&self) -> u64 {
        self.max_waiting_requests
    }

    pub const fn fallback_on_rate_limit(&self) -> bool {
        self.fallback_on_rate_limit
    }
}
