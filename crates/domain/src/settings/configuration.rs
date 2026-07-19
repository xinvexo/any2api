use std::collections::BTreeMap;

use super::definition::{
    SaturationMode, SettingKey, SettingValue, SettingsValidationError, validate_value,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SettingOverrides(BTreeMap<SettingKey, SettingValue>);

impl SettingOverrides {
    pub fn from_entries(
        entries: impl IntoIterator<Item = (SettingKey, SettingValue)>,
    ) -> Result<Self, SettingsValidationError> {
        let mut values = Self::default();
        for (key, value) in entries {
            values.insert(key, value)?;
        }
        Ok(values)
    }

    pub fn insert(
        &mut self,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<Option<SettingValue>, SettingsValidationError> {
        validate_value(key, value)?;
        Ok(self.0.insert(key, value))
    }

    pub fn remove(&mut self, key: SettingKey) -> Option<SettingValue> {
        self.0.remove(&key)
    }

    pub fn get(&self, key: SettingKey) -> Option<SettingValue> {
        self.0.get(&key).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (SettingKey, SettingValue)> + '_ {
        self.0.iter().map(|(key, value)| (*key, *value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerSettings {
    on_saturated: SaturationMode,
    queue_timeout_ms: u64,
    max_waiting_requests: u64,
    fallback_on_saturation: bool,
    auxiliary_global_concurrency: u64,
    auxiliary_per_credential_concurrency: u64,
}

impl SchedulerSettings {
    pub const fn on_saturated(&self) -> SaturationMode {
        self.on_saturated
    }

    pub const fn queue_timeout_ms(&self) -> u64 {
        self.queue_timeout_ms
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsConfiguration {
    overrides: SettingOverrides,
    scheduler: SchedulerSettings,
}

impl SettingsConfiguration {
    pub fn from_overrides(overrides: SettingOverrides) -> Result<Self, SettingsValidationError> {
        let value = |key| overrides.get(key).unwrap_or(key.definition().default());
        let on_saturated = match value(SettingKey::SchedulerOnSaturated) {
            SettingValue::Saturation(value) => value,
            _ => return Err(SettingsValidationError::InvalidType),
        };
        let scheduler = SchedulerSettings {
            on_saturated,
            queue_timeout_ms: integer(value(SettingKey::SchedulerQueueTimeout))?,
            max_waiting_requests: integer(value(SettingKey::SchedulerMaxWaitingRequests))?,
            fallback_on_saturation: boolean(value(SettingKey::SchedulerFallbackOnSaturation))?,
            auxiliary_global_concurrency: integer(value(
                SettingKey::SchedulerAuxiliaryGlobalConcurrency,
            ))?,
            auxiliary_per_credential_concurrency: integer(value(
                SettingKey::SchedulerAuxiliaryPerCredentialConcurrency,
            ))?,
        };
        Ok(Self {
            overrides,
            scheduler,
        })
    }

    #[must_use]
    pub fn defaults() -> Self {
        Self::from_overrides(SettingOverrides::default()).expect("valid defaults")
    }

    pub const fn scheduler(&self) -> &SchedulerSettings {
        &self.scheduler
    }

    pub const fn overrides(&self) -> &SettingOverrides {
        &self.overrides
    }

    pub fn override_value(&self, key: SettingKey) -> Option<SettingValue> {
        self.overrides.get(key)
    }

    pub fn effective_value(&self, key: SettingKey) -> SettingValue {
        match key {
            SettingKey::SchedulerOnSaturated => {
                SettingValue::Saturation(self.scheduler.on_saturated)
            }
            SettingKey::SchedulerQueueTimeout => {
                SettingValue::DurationMs(self.scheduler.queue_timeout_ms)
            }
            SettingKey::SchedulerMaxWaitingRequests => {
                SettingValue::Integer(self.scheduler.max_waiting_requests)
            }
            SettingKey::SchedulerFallbackOnSaturation => {
                SettingValue::Boolean(self.scheduler.fallback_on_saturation)
            }
            SettingKey::SchedulerAuxiliaryGlobalConcurrency => {
                SettingValue::Integer(self.scheduler.auxiliary_global_concurrency)
            }
            SettingKey::SchedulerAuxiliaryPerCredentialConcurrency => {
                SettingValue::Integer(self.scheduler.auxiliary_per_credential_concurrency)
            }
        }
    }
}

fn integer(value: SettingValue) -> Result<u64, SettingsValidationError> {
    match value {
        SettingValue::Integer(value) | SettingValue::DurationMs(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

fn boolean(value: SettingValue) -> Result<bool, SettingsValidationError> {
    match value {
        SettingValue::Boolean(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{SettingOverrides, SettingsConfiguration};
    use crate::{SaturationMode, SettingKey, SettingValue, SettingsValidationError};

    #[test]
    fn defaults_match_architecture() {
        let settings = SettingsConfiguration::defaults();
        assert_eq!(settings.scheduler().on_saturated(), SaturationMode::Wait);
        assert_eq!(settings.scheduler().queue_timeout_ms(), 30_000);
        assert_eq!(settings.scheduler().max_waiting_requests(), 128);
        assert_eq!(settings.scheduler().auxiliary_global_concurrency(), 32);
        assert_eq!(
            settings.scheduler().auxiliary_per_credential_concurrency(),
            4
        );
        for key in SettingKey::ALL {
            let definition = key.definition();
            assert_eq!(definition.apply_mode().as_str(), "hot_reload");
        }
        assert_eq!(
            SettingKey::SchedulerOnSaturated
                .definition()
                .allowed_values(),
            ["wait", "reject"]
        );
    }

    #[test]
    fn values_round_trip_and_validate_bounds() {
        let key = SettingKey::SchedulerQueueTimeout;
        let value = SettingValue::from_json(key, &json!(5000)).expect("duration");
        assert_eq!(value, SettingValue::DurationMs(5000));
        assert!(SettingValue::from_json(key, &json!(0)).is_err());
        let key = SettingKey::SchedulerOnSaturated;
        assert_eq!(
            SettingValue::from_json(key, &json!(true)),
            Err(SettingsValidationError::InvalidType)
        );
        assert_eq!(
            SettingValue::from_json(key, &json!("nope")),
            Err(SettingsValidationError::InvalidEnum)
        );
    }

    #[test]
    fn overrides_compile_into_effective_scheduler_settings() {
        let mut overrides = SettingOverrides::default();
        overrides
            .insert(
                SettingKey::SchedulerFallbackOnSaturation,
                SettingValue::Boolean(true),
            )
            .expect("override");
        let settings = SettingsConfiguration::from_overrides(overrides).expect("settings");
        assert!(settings.scheduler().fallback_on_saturation());
        assert_eq!(
            settings.effective_value(SettingKey::SchedulerFallbackOnSaturation),
            SettingValue::Boolean(true)
        );
    }
}
