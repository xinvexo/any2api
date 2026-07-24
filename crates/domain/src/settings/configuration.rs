use std::collections::BTreeMap;

use super::{
    AdminSettings, AffinitySettings, LoggingSettings, OAuthSettings, ReliabilitySettings,
    SchedulerSettings, SettingKey, SettingValue, SettingsValidationError, ShutdownSettings,
    StreamSettings, UpstreamSettings, value::validate_value,
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
    logging: LoggingSettings,
    oauth: OAuthSettings,
    upstream: UpstreamSettings,
    stream: StreamSettings,
    shutdown: ShutdownSettings,
}

impl SettingsConfiguration {
    pub fn from_overrides(overrides: SettingOverrides) -> Result<Self, SettingsValidationError> {
        let scheduler = SchedulerSettings::from_overrides(&overrides)?;
        let affinity = AffinitySettings::from_overrides(&overrides)?;
        let reliability = ReliabilitySettings::from_overrides(&overrides)?;
        let admin = AdminSettings::from_overrides(&overrides)?;
        let logging = LoggingSettings::from_overrides(&overrides)?;
        let oauth = OAuthSettings::from_overrides(&overrides)?;
        let upstream = UpstreamSettings::from_overrides(&overrides)?;
        let stream = StreamSettings::from_overrides(&overrides)?;
        let shutdown = ShutdownSettings::from_overrides(&overrides)?;
        Ok(Self {
            overrides,
            scheduler,
            affinity,
            reliability,
            admin,
            logging,
            oauth,
            upstream,
            stream,
            shutdown,
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

    pub const fn logging(&self) -> &LoggingSettings {
        &self.logging
    }

    pub const fn oauth(&self) -> &OAuthSettings {
        &self.oauth
    }

    pub const fn stream(&self) -> &StreamSettings {
        &self.stream
    }

    pub const fn upstream(&self) -> &UpstreamSettings {
        &self.upstream
    }

    pub const fn shutdown(&self) -> &ShutdownSettings {
        &self.shutdown
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
    use crate::{
        AffinityMode, FileLogLevel, SaturationMode, SettingKey, SettingValue, SettingValueType,
        SettingsValidationError,
    };

    #[test]
    fn defaults_match_architecture() {
        let settings = SettingsConfiguration::defaults();
        assert_eq!(settings.scheduler().on_saturated(), SaturationMode::Wait);
        assert_eq!(settings.scheduler().queue_timeout_secs(), 30);
        assert_eq!(settings.scheduler().max_waiting_requests(), 128);
        assert_eq!(settings.scheduler().auxiliary_global_concurrency(), 32);
        assert_eq!(
            settings.scheduler().auxiliary_per_credential_concurrency(),
            4
        );
        assert!(settings.affinity().soft_enabled());
        assert_eq!(settings.affinity().soft_mode(), AffinityMode::Prefer);
        assert_eq!(settings.affinity().soft_ttl_secs(), 3_600);
        assert_eq!(settings.affinity().hard_ttl_secs(), 86_400);
        assert_eq!(settings.affinity().soft_prefer_wait_timeout_secs(), 2);
        assert_eq!(settings.affinity().fixed_wait_timeout_secs(), 30);
        assert_eq!(settings.reliability().max_total_attempts(), 3);
        assert_eq!(settings.reliability().max_credential_switches(), 2);
        assert_eq!(settings.reliability().max_same_credential_retries(), 1);
        assert_eq!(settings.reliability().precommit_total_budget_secs(), 20);
        assert_eq!(settings.reliability().endpoint_failure_threshold(), 3);
        assert_eq!(settings.reliability().proxy_open_duration_secs(), 30);
        assert!(!settings.admin().remote_enabled());
        assert_eq!(settings.admin().session_idle_timeout_secs(), 43_200);
        assert_eq!(settings.admin().session_absolute_timeout_secs(), 604_800);
        assert_eq!(settings.admin().login_failure_window_secs(), 900);
        assert_eq!(settings.admin().login_max_failures(), 5);
        assert!(settings.logging().request_enabled());
        assert_eq!(settings.logging().request_retention_secs(), 2_592_000);
        assert_eq!(settings.logging().request_max_rows(), 200_000);
        assert_eq!(settings.logging().file_level(), FileLogLevel::Info);
        assert_eq!(settings.logging().file_retention_secs(), 604_800);
        assert_eq!(settings.logging().file_max_total_size(), 256 * 1024 * 1024);
        assert_eq!(settings.logging().telemetry_queue_capacity(), 4_096);
        assert_eq!(settings.oauth().refresh_scan_interval_secs(), 30);
        assert_eq!(settings.oauth().refresh_lead_time_secs(), 300);
        assert_eq!(settings.upstream().read_timeout_secs(), 15);
        assert!(!settings.upstream().strict_ssrf());
        assert_eq!(settings.stream().precommit_max_bytes(), 256 * 1024);
        assert_eq!(settings.stream().precommit_max_duration_secs(), 5);
        assert_eq!(settings.stream().postcommit_idle_timeout_secs(), 60);
        assert_eq!(settings.shutdown().request_grace_period_secs(), 30);
        assert_eq!(settings.shutdown().finalize_timeout_secs(), 5);
        assert!(
            SettingKey::ALL
                .into_iter()
                .all(|key| key.definition().apply_mode().as_str() == "hot_reload")
        );
    }

    #[test]
    fn values_round_trip_and_validate_bounds_and_enum_domains() {
        let key = SettingKey::SchedulerQueueTimeout;
        let value = SettingValue::from_json(key, &json!(5)).expect("duration");
        assert_eq!(value, SettingValue::DurationSecs(5));
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
            SettingValue::from_json(SettingKey::LogsFileLevel, &json!("debug")),
            Ok(SettingValue::FileLogLevel(FileLogLevel::Debug))
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
    fn timeout_and_stream_budget_definitions_match_the_public_contract() {
        let bytes = SettingKey::StreamPrecommitMaxBytes.definition();
        assert_eq!(bytes.value_type(), SettingValueType::Integer);
        assert_eq!(bytes.default(), SettingValue::Integer(256 * 1024));
        assert_eq!(bytes.min(), Some(SettingValue::Integer(1)));
        assert_eq!(bytes.max(), Some(SettingValue::Integer(16 * 1024 * 1024)));
        assert!(bytes.description().contains("每个 SSE 帧"));

        let duration = SettingKey::StreamPrecommitMaxDuration.definition();
        assert_eq!(duration.value_type(), SettingValueType::DurationSecs);
        assert_eq!(duration.default(), SettingValue::DurationSecs(5));
        assert_eq!(duration.min(), Some(SettingValue::DurationSecs(1)));
        assert_eq!(duration.max(), Some(SettingValue::DurationSecs(86_400)));
        let postcommit = SettingKey::StreamPostcommitIdleTimeout.definition();
        assert_eq!(postcommit.value_type(), SettingValueType::DurationSecs);
        assert_eq!(postcommit.default(), SettingValue::DurationSecs(60));
        assert_eq!(postcommit.min(), Some(SettingValue::DurationSecs(1)));
        assert_eq!(postcommit.max(), Some(SettingValue::DurationSecs(86_400)));
        let read_timeout = SettingKey::UpstreamReadTimeout.definition();
        assert_eq!(read_timeout.value_type(), SettingValueType::DurationSecs);
        assert_eq!(read_timeout.default(), SettingValue::DurationSecs(15));
        assert_eq!(read_timeout.min(), Some(SettingValue::DurationSecs(1)));
        assert_eq!(read_timeout.max(), Some(SettingValue::DurationSecs(86_400)));
        assert_eq!(SettingKey::parse("stream.precommit.max_events"), None);
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
            (
                SettingKey::UpstreamReadTimeout,
                SettingValue::DurationSecs(2),
            ),
            (SettingKey::UpstreamStrictSsrf, SettingValue::Boolean(true)),
            (
                SettingKey::StreamPostcommitIdleTimeout,
                SettingValue::DurationSecs(3),
            ),
        ])
        .expect("overrides");
        let settings = SettingsConfiguration::from_overrides(overrides).expect("settings");
        assert!(settings.scheduler().fallback_on_saturation());
        assert_eq!(settings.affinity().soft_mode(), AffinityMode::Strict);
        assert_eq!(settings.upstream().read_timeout_secs(), 2);
        assert!(settings.upstream().strict_ssrf());
        assert_eq!(settings.stream().postcommit_idle_timeout_secs(), 3);
        assert_eq!(
            settings.effective_value(SettingKey::AffinitySoftMode),
            SettingValue::AffinityMode(AffinityMode::Strict)
        );
    }

    #[test]
    fn reliability_rejects_an_inverted_delay_range() {
        let overrides = SettingOverrides::from_entries([
            (SettingKey::RetryBaseDelay, SettingValue::DurationSecs(2)),
            (SettingKey::RetryMaxDelay, SettingValue::DurationSecs(0)),
        ])
        .expect("individual values are valid");

        assert_eq!(
            SettingsConfiguration::from_overrides(overrides),
            Err(SettingsValidationError::InvalidCombination)
        );
    }

    #[test]
    fn oauth_refresh_rejects_a_lead_time_shorter_than_the_scan_interval() {
        let overrides = SettingOverrides::from_entries([
            (
                SettingKey::OAuthRefreshScanInterval,
                SettingValue::DurationSecs(60),
            ),
            (
                SettingKey::OAuthRefreshLeadTime,
                SettingValue::DurationSecs(30),
            ),
        ])
        .expect("individual values are valid");

        assert_eq!(
            SettingsConfiguration::from_overrides(overrides),
            Err(SettingsValidationError::InvalidCombination)
        );
    }
}
