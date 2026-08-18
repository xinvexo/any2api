use std::collections::BTreeMap;

use super::{
    AdminSettings, AffinitySettings, LoggingSettings, ModelSettings, NetworkSettings,
    OAuthSettings, ReliabilitySettings, SchedulerSettings, SettingKey, SettingValue,
    SettingsValidationError, ShutdownSettings, StreamSettings, UpstreamSettings,
    value::normalize_value,
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
        let value = normalize_value(key, value)?;
        Ok(self.0.insert(key, value))
    }

    pub fn remove(&mut self, key: SettingKey) -> Option<SettingValue> {
        self.0.remove(&key)
    }

    pub fn get(&self, key: SettingKey) -> Option<SettingValue> {
        self.0.get(&key).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (SettingKey, SettingValue)> + '_ {
        self.0.iter().map(|(key, value)| (*key, value.clone()))
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
    models: ModelSettings,
    network: NetworkSettings,
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
        let models = ModelSettings::from_overrides(&overrides)?;
        let network = NetworkSettings::from_overrides(&overrides)?;
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
            models,
            network,
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

    pub const fn models(&self) -> &ModelSettings {
        &self.models
    }

    pub const fn network(&self) -> &NetworkSettings {
        &self.network
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
    use crate::{FileLogLevel, ModelAccess, SettingKey, SettingValue, SettingsValidationError};

    #[test]
    fn values_round_trip_and_validate_bounds_and_enum_domains() {
        let key = SettingKey::SchedulerQueueTimeout;
        let value = SettingValue::from_json(key, &json!(5)).expect("duration");
        assert_eq!(value, SettingValue::DurationSecs(5));
        assert!(SettingValue::from_json(key, &json!(0)).is_err());
        assert_eq!(
            SettingValue::from_json(SettingKey::AffinityTtl, &json!(true)),
            Err(SettingsValidationError::InvalidType)
        );
        assert_eq!(
            SettingValue::from_json(SettingKey::LogsFileLevel, &json!("debug")),
            Ok(SettingValue::FileLogLevel(FileLogLevel::Debug))
        );
        assert_eq!(
            SettingOverrides::from_entries([(
                SettingKey::SchedulerOnRateLimited,
                SettingValue::FileLogLevel(FileLogLevel::Info),
            )]),
            Err(SettingsValidationError::InvalidEnum)
        );
        assert_eq!(
            SettingValue::from_json(
                SettingKey::ModelsAllowed,
                &json!(["z-model", "a-model", "z-model"]),
            ),
            Ok(SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
                "a-model".to_owned(),
                "z-model".to_owned(),
            ])))
        );
        assert_eq!(
            SettingValue::from_json(SettingKey::ModelsAllowed, &serde_json::Value::Null),
            Err(SettingsValidationError::InvalidType)
        );
        assert_eq!(
            SettingValue::from_json(SettingKey::ModelsAllowed, &json!([])),
            Ok(SettingValue::ModelAccess(
                ModelAccess::Allowlist(Vec::new())
            ))
        );
        assert_eq!(
            SettingValue::from_json(SettingKey::ModelsAllowed, &json!("all")),
            Ok(SettingValue::ModelAccess(ModelAccess::All))
        );
        assert_eq!(
            SettingValue::from_json(SettingKey::ModelsAllowed, &json!([" invalid "])),
            Err(SettingsValidationError::InvalidListValue)
        );
        assert_eq!(
            SettingValue::from_json(
                SettingKey::NetworkTrustedProxyCidrs,
                &json!([" 127.0.0.1 ", "10.0.3.4/8", "127.0.0.1/32"]),
            ),
            Ok(SettingValue::StringList(vec![
                "10.0.0.0/8".to_owned(),
                "127.0.0.1/32".to_owned(),
            ]))
        );
        assert_eq!(
            SettingValue::from_json(
                SettingKey::NetworkTrustedProxyCidrs,
                &json!(["not-an-address"]),
            ),
            Err(SettingsValidationError::InvalidListValue)
        );
    }

    #[test]
    fn overrides_compile_into_effective_settings() {
        let overrides = SettingOverrides::from_entries([
            (
                SettingKey::SchedulerFallbackOnRateLimit,
                SettingValue::Boolean(true),
            ),
            (SettingKey::AffinityEnabled, SettingValue::Boolean(true)),
            (SettingKey::AffinityTtl, SettingValue::DurationSecs(120)),
            (
                SettingKey::NetworkMaxConnections,
                SettingValue::Integer(512),
            ),
            (
                SettingKey::NetworkRequestHeaderTimeout,
                SettingValue::DurationSecs(12),
            ),
            (
                SettingKey::NetworkRequestBodyIdleTimeout,
                SettingValue::DurationSecs(34),
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
        assert!(settings.scheduler().fallback_on_rate_limit());
        assert!(settings.affinity().enabled());
        assert_eq!(settings.affinity().ttl_secs(), 120);
        assert_eq!(settings.network().max_connections().get(), 512);
        assert_eq!(settings.network().request_header_timeout_secs(), 12);
        assert_eq!(settings.network().request_body_idle_timeout_secs(), 34);
        assert_eq!(settings.upstream().read_timeout_secs(), 2);
        assert!(settings.upstream().strict_ssrf());
        assert_eq!(settings.stream().postcommit_idle_timeout_secs(), 3);
        assert_eq!(
            settings.effective_value(SettingKey::AffinityTtl),
            SettingValue::DurationSecs(120)
        );
    }

    #[test]
    fn inbound_connection_limit_uses_restart_required_bounded_integer_metadata() {
        let definition = SettingKey::NetworkMaxConnections.definition();
        assert_eq!(definition.default(), SettingValue::Integer(4_096));
        assert_eq!(definition.min(), Some(SettingValue::Integer(1)));
        assert_eq!(definition.max(), Some(SettingValue::Integer(100_000)));
        assert_eq!(
            definition.apply_mode(),
            crate::SettingApplyMode::RestartRequired
        );
        assert_eq!(
            SettingsConfiguration::defaults()
                .network()
                .max_connections()
                .get(),
            4_096
        );
        assert_eq!(
            SettingOverrides::from_entries([(
                SettingKey::NetworkMaxConnections,
                SettingValue::Integer(0),
            )]),
            Err(SettingsValidationError::OutOfRange)
        );
        assert_eq!(
            SettingOverrides::from_entries([(
                SettingKey::NetworkMaxConnections,
                SettingValue::Integer(100_001),
            )]),
            Err(SettingsValidationError::OutOfRange)
        );
    }

    #[test]
    fn inbound_slowloris_timeouts_are_bounded_and_restart_required() {
        for (key, default, max) in [
            (
                SettingKey::NetworkRequestHeaderTimeout,
                30,
                crate::settings::definition::MAX_SETTING_DURATION_SECS,
            ),
            (
                SettingKey::NetworkRequestBodyIdleTimeout,
                60,
                crate::settings::definition::MAX_SETTING_DURATION_SECS,
            ),
        ] {
            let definition = key.definition();
            assert_eq!(definition.default(), SettingValue::DurationSecs(default));
            assert_eq!(definition.min(), Some(SettingValue::DurationSecs(1)));
            assert_eq!(definition.max(), Some(SettingValue::DurationSecs(max)));
            assert_eq!(
                definition.apply_mode(),
                crate::SettingApplyMode::RestartRequired
            );
        }
        assert_eq!(
            SettingsConfiguration::defaults()
                .network()
                .request_header_timeout_secs(),
            30
        );
        assert_eq!(
            SettingsConfiguration::defaults()
                .network()
                .request_body_idle_timeout_secs(),
            60
        );
    }

    #[test]
    fn reliability_compiles_the_precommit_total_budget_override() {
        let settings = SettingsConfiguration::from_overrides(
            SettingOverrides::from_entries([(
                SettingKey::RetryPrecommitTotalBudget,
                SettingValue::DurationSecs(42),
            )])
            .expect("valid override"),
        )
        .expect("settings");

        assert_eq!(settings.reliability().precommit_total_budget_secs(), 42);
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
