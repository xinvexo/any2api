use std::sync::Arc;

use any2api_domain::{ConfigRevision, ProxyDraft, ProxyProfileId};

use super::ConfigPublisher;
use crate::{
    config_command::ConfigCommand, config_publish_error::ConfigPublishError,
    proxy_password_secret::ProxyPasswordSecret, published_snapshot::PublishedSnapshot,
};

impl ConfigPublisher {
    pub async fn create_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::CreateProxy { id, draft })
            .await
    }

    pub async fn update_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        draft: ProxyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::UpdateProxy { id, draft })
            .await
    }

    pub async fn delete_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::DeleteProxy { id })
            .await
    }

    pub async fn set_global_proxy(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::SetGlobalProxy { id })
            .await
    }

    pub async fn set_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
        username: String,
        password: ProxyPasswordSecret,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::SetProxyAuthentication {
                id,
                username,
                password,
            },
        )
        .await
    }

    pub async fn clear_proxy_authentication(
        &self,
        expected: ConfigRevision,
        id: ProxyProfileId,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(expected, ConfigCommand::ClearProxyAuthentication { id })
            .await
    }
}
