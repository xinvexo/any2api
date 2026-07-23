use std::sync::Arc;

use any2api_domain::{ConfigRevision, SettingKey, SettingValue};

use super::ConfigPublisher;
use crate::{
    config_command::ConfigCommand, config_publish_error::ConfigPublishError,
    published_snapshot::PublishedSnapshot,
};

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
}
