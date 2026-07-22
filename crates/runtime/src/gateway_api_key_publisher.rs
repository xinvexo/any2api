use std::sync::Arc;

use any2api_domain::{ConfigRevision, GatewayApiKeyDraft, GatewayApiKeyId};

use crate::{
    config_publish_error::ConfigPublishError, gateway_api_key_token::GatewayApiKeyToken,
    published_snapshot::PublishedSnapshot, publisher::ConfigPublisher,
};

pub struct GatewayApiKeyPublishResult {
    snapshot: Arc<PublishedSnapshot>,
    token: GatewayApiKeyToken,
}

impl GatewayApiKeyPublishResult {
    #[must_use]
    pub fn snapshot(&self) -> &PublishedSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub const fn token(&self) -> &GatewayApiKeyToken {
        &self.token
    }
}

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
    Revoke {
        id: GatewayApiKeyId,
        expected_config_version: u64,
    },
}

struct GatewayApiKeyPublishOutcome {
    snapshot: Arc<PublishedSnapshot>,
    token: Option<GatewayApiKeyToken>,
}

impl ConfigPublisher {
    pub async fn create_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        draft: GatewayApiKeyDraft,
    ) -> Result<GatewayApiKeyPublishResult, ConfigPublishError> {
        let token = generate_token()?;
        let outcome = self
            .publish_gateway_api_key(
                expected,
                GatewayApiKeyPublishCommand::Create { id, draft, token },
            )
            .await?;
        Ok(GatewayApiKeyPublishResult {
            snapshot: outcome.snapshot,
            token: outcome
                .token
                .expect("create gateway API Key must return its generated token"),
        })
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
        .map(|outcome| outcome.snapshot)
    }

    pub async fn rotate_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
        expected_token_version: u64,
    ) -> Result<GatewayApiKeyPublishResult, ConfigPublishError> {
        let token = generate_token()?;
        let outcome = self
            .publish_gateway_api_key(
                expected,
                GatewayApiKeyPublishCommand::Rotate {
                    id,
                    expected_config_version,
                    expected_token_version,
                    token,
                },
            )
            .await?;
        Ok(GatewayApiKeyPublishResult {
            snapshot: outcome.snapshot,
            token: outcome
                .token
                .expect("gateway API Key rotation must return its generated token"),
        })
    }

    pub async fn revoke_gateway_api_key(
        &self,
        expected: ConfigRevision,
        id: GatewayApiKeyId,
        expected_config_version: u64,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish_gateway_api_key(
            expected,
            GatewayApiKeyPublishCommand::Revoke {
                id,
                expected_config_version,
            },
        )
        .await
        .map(|outcome| outcome.snapshot)
    }

    async fn publish_gateway_api_key(
        &self,
        expected: ConfigRevision,
        command: GatewayApiKeyPublishCommand,
    ) -> Result<GatewayApiKeyPublishOutcome, ConfigPublishError> {
        let publisher = self.clone();
        crate::publish_task::run(self.runtime.lifecycle(), async move {
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
    ) -> Result<GatewayApiKeyPublishOutcome, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        if current.revision() != expected {
            return Err(ConfigPublishError::RevisionConflict {
                expected,
                actual: current.revision(),
            });
        }
        let (committed, token) = match command {
            GatewayApiKeyPublishCommand::Create { id, draft, token } => {
                let committed = self
                    .repository
                    .create_gateway_api_key(expected, id, draft, token.storage_secret())
                    .await?;
                (committed, Some(token))
            }
            GatewayApiKeyPublishCommand::Update {
                id,
                expected_config_version,
                draft,
            } => (
                self.repository
                    .update_gateway_api_key(expected, id, expected_config_version, draft)
                    .await?,
                None,
            ),
            GatewayApiKeyPublishCommand::Rotate {
                id,
                expected_config_version,
                expected_token_version,
                token,
            } => {
                let committed = self
                    .repository
                    .rotate_gateway_api_key(
                        expected,
                        id,
                        expected_config_version,
                        expected_token_version,
                        token.storage_secret(),
                    )
                    .await?;
                (committed, Some(token))
            }
            GatewayApiKeyPublishCommand::Revoke {
                id,
                expected_config_version,
            } => (
                self.repository
                    .revoke_gateway_api_key(expected, id, expected_config_version)
                    .await?,
                None,
            ),
        };
        Ok(GatewayApiKeyPublishOutcome {
            snapshot: self.publish_committed(current, expected, committed),
            token,
        })
    }
}

fn generate_token() -> Result<GatewayApiKeyToken, ConfigPublishError> {
    GatewayApiKeyToken::generate().map_err(|_| ConfigPublishError::GatewayApiKeyTokenGeneration)
}
