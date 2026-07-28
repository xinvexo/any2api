use std::sync::Arc;

use any2api_domain::{ConfigRevision, SettingKey, SettingValue, SettingsValidationError};

use super::ConfigPublisher;
use crate::configuration::{ConfigPublishError, PublishedSnapshot, command::ConfigCommand};

impl ConfigPublisher {
    pub async fn set_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        if key == SettingKey::ModelsAllowed && value == SettingValue::OptionalStringList(None) {
            return self.reset_setting_override(expected, key).await;
        }
        self.publish(expected, ConfigCommand::SetSettingOverride { key, value })
            .await
    }

    pub async fn reset_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::ResetSettingOverride { key })
            .await
    }

    pub(super) fn validate_setting_command(
        &self,
        current: &PublishedSnapshot,
        command: &ConfigCommand,
    ) -> Result<(), ConfigPublishError> {
        let ConfigCommand::SetSettingOverride {
            key: SettingKey::ModelsAllowed,
            value: SettingValue::OptionalStringList(Some(models)),
        } = command
        else {
            return Ok(());
        };
        let published = current.published_public_model_names();
        if models.iter().any(|model| !published.contains(model)) {
            return Err(ConfigPublishError::InvalidSetting(
                SettingsValidationError::InvalidListValue,
            ));
        }
        Ok(())
    }
}
