use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, SettingKey, SettingOverrideChange, SettingValue, SettingsValidationError,
};

use super::ConfigPublisher;
use crate::configuration::{ConfigPublishError, PublishedSnapshot, command::ConfigCommand};

impl ConfigPublisher {
    pub async fn set_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
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

    pub async fn apply_setting_changes(
        &self,
        expected: ConfigRevision,
        changes: Vec<SettingOverrideChange>,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::ApplySettingChanges { changes })
            .await
    }

    pub(super) fn validate_setting_command(
        &self,
        current: &PublishedSnapshot,
        command: &ConfigCommand,
    ) -> Result<(), ConfigPublishError> {
        match command {
            ConfigCommand::SetSettingOverride { key, value } => {
                validate_model_allowlist(current, *key, value)?;
            }
            ConfigCommand::ApplySettingChanges { changes } => {
                for change in changes {
                    if let SettingOverrideChange::Set { key, value } = change {
                        validate_model_allowlist(current, *key, value)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn validate_model_allowlist(
    current: &PublishedSnapshot,
    key: SettingKey,
    value: &SettingValue,
) -> Result<(), ConfigPublishError> {
    let (SettingKey::ModelsAllowed, SettingValue::StringList(models)) = (key, value) else {
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
