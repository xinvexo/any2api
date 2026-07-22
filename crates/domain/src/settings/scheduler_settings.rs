use super::{
    SaturationMode, SettingKey, SettingOverrides, SettingValue, SettingsValidationError,
    value::integer,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerSettings {
    on_saturated: SaturationMode,
    queue_timeout_secs: u64,
    max_waiting_requests: u64,
    fallback_on_saturation: bool,
    auxiliary_global_concurrency: u64,
    auxiliary_per_credential_concurrency: u64,
}

impl SchedulerSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.effective_value(key);
        let on_saturated = match value(SettingKey::SchedulerOnSaturated) {
            SettingValue::Saturation(value) => value,
            _ => return Err(SettingsValidationError::InvalidType),
        };
        Ok(Self {
            on_saturated,
            queue_timeout_secs: integer(value(SettingKey::SchedulerQueueTimeout))?,
            max_waiting_requests: integer(value(SettingKey::SchedulerMaxWaitingRequests))?,
            fallback_on_saturation: match value(SettingKey::SchedulerFallbackOnSaturation) {
                SettingValue::Boolean(value) => value,
                _ => return Err(SettingsValidationError::InvalidType),
            },
            auxiliary_global_concurrency: integer(value(
                SettingKey::SchedulerAuxiliaryGlobalConcurrency,
            ))?,
            auxiliary_per_credential_concurrency: integer(value(
                SettingKey::SchedulerAuxiliaryPerCredentialConcurrency,
            ))?,
        })
    }

    pub const fn on_saturated(&self) -> SaturationMode {
        self.on_saturated
    }

    pub const fn queue_timeout_secs(&self) -> u64 {
        self.queue_timeout_secs
    }

    pub const fn max_waiting_requests(&self) -> u64 {
        self.max_waiting_requests
    }

    pub const fn fallback_on_saturation(&self) -> bool {
        self.fallback_on_saturation
    }

    pub const fn auxiliary_global_concurrency(&self) -> u64 {
        self.auxiliary_global_concurrency
    }

    pub const fn auxiliary_per_credential_concurrency(&self) -> u64 {
        self.auxiliary_per_credential_concurrency
    }
}
