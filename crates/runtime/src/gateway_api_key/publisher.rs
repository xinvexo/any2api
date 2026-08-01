use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};

use crate::{
    configuration::{
        ConfigPublishError, ConfigPublisher, PublishedSnapshot, command::ConfigCommand,
    },
    gateway_api_key::token::GatewayApiKeyToken,
};

impl ConfigPublisher {
    pub async fn create_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let token = GatewayApiKeyToken::generate()
            .map_err(|_| ConfigPublishError::GatewayApiKeyTokenGeneration)?;
        self.publish(
            expected,
            ConfigCommand::CreateGatewayApiKey {
                id,
                draft,
                token: token.storage_secret(),
            },
        )
        .await
    }

    pub async fn update_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        draft: GatewayApiKeyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::UpdateGatewayApiKey {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    pub async fn rotate_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let token = GatewayApiKeyToken::generate()
            .map_err(|_| ConfigPublishError::GatewayApiKeyTokenGeneration)?;
        self.publish(
            expected,
            ConfigCommand::RotateGatewayApiKey {
                id,
                expected_config_version,
                expected_token_version,
                token: token.storage_secret(),
            },
        )
        .await
    }

    pub async fn delete_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::DeleteGatewayApiKey {
                id,
                expected_config_version,
            },
        )
        .await
    }
}
