use super::{SettingKey, SettingOverrides, SettingsValidationError, value::integer};

pub const MAX_STREAM_PRECOMMIT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamSettings {
    precommit_max_bytes: u64,
    precommit_max_duration_secs: u64,
    postcommit_idle_timeout_secs: u64,
}

impl StreamSettings {
    pub(super) fn from_overrides(
        overrides: &SettingOverrides,
    ) -> Result<Self, SettingsValidationError> {
        let value = |key| integer(overrides.effective_value(key));
        Ok(Self {
            precommit_max_bytes: value(SettingKey::StreamPrecommitMaxBytes)?,
            precommit_max_duration_secs: value(SettingKey::StreamPrecommitMaxDuration)?,
            postcommit_idle_timeout_secs: value(SettingKey::StreamPostcommitIdleTimeout)?,
        })
    }

    pub const fn precommit_max_bytes(&self) -> u64 {
        self.precommit_max_bytes
    }

    pub const fn precommit_max_duration_secs(&self) -> u64 {
        self.precommit_max_duration_secs
    }

    pub const fn postcommit_idle_timeout_secs(&self) -> u64 {
        self.postcommit_idle_timeout_secs
    }
}
