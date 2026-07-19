use std::collections::BTreeMap;

use super::{
    AdminSettings, AffinitySettings, ReliabilitySettings, SchedulerSettings, SettingKey,
    SettingValue, SettingsValidationError, value::validate_value,
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

    pub(super) fn effective_value(&self, key: SettingKey) -> SettingValue {
        self.get(key).unwrap_or(key.definition().default())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsConfiguration {
    overrides: SettingOverrides,
    scheduler: SchedulerSettings,
    affinity: AffinitySettings,
    reliability: ReliabilitySettings,
    admin: AdminSettings,
}

impl SettingsConfiguration {
    pub fn from_overrides(overrides: SettingOverrides) -> Result<Self, SettingsValidationError> {
        let scheduler = SchedulerSettings::from_overrides(&overrides)?;
        let affinity = AffinitySettings::from_overrides(&overrides)?;
        let reliability = ReliabilitySettings::from_overrides(&overrides)?;
        let admin = AdminSettings::from_overrides(&overrides)?;
        Ok(Self {
            overrides,
            scheduler,
            affinity,
            reliability,
            admin,
        })
    }

    #[must_use]
    pub fn defaults() -> Self {
        Self::from_overrides(SettingOverrides::default()).expect("valid defaults")
    }

    pub const fn scheduler(&self) -> &SchedulerSettings {
        &self.scheduler
    }

    pub const fn affinity(&self) -> &AffinitySettings {
        &self.affinity
    }

    pub const fn reliability(&self) -> &ReliabilitySettings {
        &self.reliability
    }

    pub const fn admin(&self) -> &AdminSettings {
        &self.admin
    }

    pub const fn overrides(&self) -> &SettingOverrides {
        &self.overrides
    }

    pub fn override_value(&self, key: SettingKey) -> Option<SettingValue> {
        self.overrides.get(key)
    }

    pub fn effective_value(&self, key: SettingKey) -> SettingValue {
        self.overrides.effective_value(key)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{SettingOverrides, SettingsConfiguration};
    use crate::{AffinityMode, SaturationMode, SettingKey, SettingValue, SettingsValidationError};

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
        assert!(settings.affinity().soft_enabled());
        assert_eq!(settings.affinity().soft_mode(), AffinityMode::Prefer);
        assert_eq!(settings.affinity().soft_ttl_ms(), 3_600_000);
        assert_eq!(settings.affinity().hard_ttl_ms(), 86_400_000);
        assert_eq!(settings.affinity().soft_prefer_wait_timeout_ms(), 2_000);
        assert_eq!(settings.affinity().fixed_wait_timeout_ms(), 30_000);
        assert_eq!(settings.reliability().max_total_attempts(), 3);
        assert_eq!(settings.reliability().max_credential_switches(), 2);
        assert_eq!(settings.reliability().max_same_credential_retries(), 1);
        assert_eq!(settings.reliability().precommit_total_budget_ms(), 20_000);
        assert_eq!(settings.reliability().endpoint_failure_threshold(), 3);
        assert_eq!(settings.reliability().proxy_open_duration_ms(), 30_000);
        assert!(!settings.admin().remote_enabled());
        assert_eq!(settings.admin().session_idle_timeout_ms(), 43_200_000);
        assert_eq!(settings.admin().session_absolute_timeout_ms(), 604_800_000);
        assert_eq!(settings.admin().login_failure_window_ms(), 900_000);
        assert_eq!(settings.admin().login_max_failures(), 5);
        assert!(
            SettingKey::ALL
                .into_iter()
                .all(|key| key.definition().apply_mode().as_str() == "hot_reload")
        );
    }

    #[test]
    fn values_round_trip_and_validate_bounds_and_enum_domains() {
        let key = SettingKey::SchedulerQueueTimeout;
        let value = SettingValue::from_json(key, &json!(5000)).expect("duration");
        assert_eq!(value, SettingValue::DurationMs(5000));
        assert!(SettingValue::from_json(key, &json!(0)).is_err());
        assert_eq!(
            SettingValue::from_json(SettingKey::AffinitySoftMode, &json!(true)),
            Err(SettingsValidationError::InvalidType)
        );
        assert_eq!(
            SettingValue::from_json(SettingKey::AffinitySoftMode, &json!("wait")),
            Err(SettingsValidationError::InvalidEnum)
        );
        assert_eq!(
            SettingOverrides::from_entries([(
                SettingKey::SchedulerOnSaturated,
                SettingValue::AffinityMode(AffinityMode::Prefer),
            )]),
            Err(SettingsValidationError::InvalidEnum)
        );
    }

    #[test]
    fn overrides_compile_into_effective_settings() {
        let overrides = SettingOverrides::from_entries([
            (
                SettingKey::SchedulerFallbackOnSaturation,
                SettingValue::Boolean(true),
            ),
            (
                SettingKey::AffinitySoftMode,
                SettingValue::AffinityMode(AffinityMode::Strict),
            ),
        ])
        .expect("overrides");
        let settings = SettingsConfiguration::from_overrides(overrides).expect("settings");
        assert!(settings.scheduler().fallback_on_saturation());
        assert_eq!(settings.affinity().soft_mode(), AffinityMode::Strict);
        assert_eq!(
            settings.effective_value(SettingKey::AffinitySoftMode),
            SettingValue::AffinityMode(AffinityMode::Strict)
        );
    }

    #[test]
    fn reliability_rejects_an_inverted_delay_range() {
        let overrides = SettingOverrides::from_entries([
            (SettingKey::RetryBaseDelay, SettingValue::DurationMs(2_000)),
            (SettingKey::RetryMaxDelay, SettingValue::DurationMs(250)),
        ])
        .expect("individual values are valid");

        assert_eq!(
            SettingsConfiguration::from_overrides(overrides),
            Err(SettingsValidationError::InvalidCombination)
        );
    }
}
