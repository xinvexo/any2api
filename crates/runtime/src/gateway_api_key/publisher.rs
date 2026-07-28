use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};

use crate::{
    configuration::{ConfigPublishError, ConfigPublisher, PublishedSnapshot, publish_task},
    gateway_api_key::token::GatewayApiKeyToken,
};

enum GatewayApiKeyPublishCommand {
    Create {
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
        token: GatewayApiKeyToken,
    },
    Update {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        draft: GatewayApiKeyDraft,
    },
    Rotate {
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
        token: GatewayApiKeyToken,
    },
    Delete {
        id: GatewayApiKeyId,
        expected_config_version: u64,
    },
}

impl ConfigPublisher {
    pub async fn create_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let token = GatewayApiKeyToken::generate()
            .map_err(|_| ConfigPublishError::GatewayApiKeyTokenGeneration)?;
        self.publish_gateway_api_key(
            expected,
            GatewayApiKeyPublishCommand::Create { id, draft, token },
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
        self.publish_gateway_api_key(
            expected,
            GatewayApiKeyPublishCommand::Update {
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
        self.publish_gateway_api_key(
            expected,
            GatewayApiKeyPublishCommand::Rotate {
                id,
                expected_config_version,
                expected_token_version,
                token,
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
        self.publish_gateway_api_key(
            expected,
            GatewayApiKeyPublishCommand::Delete {
                id,
                expected_config_version,
            },
        )
        .await
    }

    async fn publish_gateway_api_key(
        &self,
        expected: ConfigRevision,
        command: GatewayApiKeyPublishCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher
                .publish_gateway_api_key_serialized(expected, command)
                .await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn publish_gateway_api_key_serialized(
        &self,
        expected: ConfigRevision,
        command: GatewayApiKeyPublishCommand,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        if current.revision() != expected {
            return Err(ConfigPublishError::RevisionConflict {
                expected,
                actual: current.revision(),
            });
        }
        let committed = match command {
            GatewayApiKeyPublishCommand::Create { id, draft, token } => {
                self.repository
                    .create_gateway_api_key(expected, id, draft, token.storage_secret())
                    .await?
            }
            GatewayApiKeyPublishCommand::Update {
                id,
                expected_config_version,
                draft,
            } => {
                self.repository
                    .update_gateway_api_key(expected, id, expected_config_version, draft)
                    .await?
            }
            GatewayApiKeyPublishCommand::Rotate {
                id,
                expected_config_version,
                expected_token_version,
                token,
            } => {
                self.repository
                    .rotate_gateway_api_key(
                        expected,
                        id,
                        expected_config_version,
                        expected_token_version,
                        token.storage_secret(),
                    )
                    .await?
            }
            GatewayApiKeyPublishCommand::Delete {
                id,
                expected_config_version,
            } => {
                self.repository
                    .delete_gateway_api_key(expected, id, expected_config_version)
                    .await?
            }
        };
        Ok(self.publish_committed(current, expected, committed))
    }
}
