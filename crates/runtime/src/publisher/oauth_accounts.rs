use std::sync::Arc;

use any2api_domain::{ConfigRevision, OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_storage::api::OAuthAccountDocument;

use super::ConfigPublisher;
use crate::{
    config_command::ConfigCommand, config_publish_error::ConfigPublishError,
    published_snapshot::PublishedSnapshot,
};

impl ConfigPublisher {
    pub(super) fn validate_oauth_account_models(
        &self,
        current: &PublishedSnapshot,
        id: OAuthAccountId,
        models: &[String],
    ) -> Result<(), ConfigPublishError> {
        current
            .oauth_accounts()
            .get(id)
            .ok_or(ConfigPublishError::OAuthAccountNotFound)?;
        let available = current
            .oauth_available_models(id)
            .ok_or(ConfigPublishError::OAuthAccountNotFound)?;
        if models.iter().all(|model| {
            available
                .binary_search_by_key(&model.as_str(), |available| available.as_str())
                .is_ok()
        }) {
            Ok(())
        } else {
            Err(ConfigPublishError::OAuthModelUnavailable)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn activate_oauth_account(
        &self,
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish_current(ConfigCommand::CreateOAuthAccount {
            id,
            provider_kind,
            draft,
            safe_account_email,
            expires_at,
            models,
            document,
        })
        .await
    }

    pub async fn update_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::UpdateOAuthAccount {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    pub async fn set_oauth_account_models(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        models: Vec<String>,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::SetOAuthAccountModels {
                id,
                expected_config_version,
                models,
            },
        )
        .await
    }

    pub(crate) async fn refresh_oauth_account(
        &self,
        id: OAuthAccountId,
        expected_token_version: u64,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        document: OAuthAccountDocument,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish_current(ConfigCommand::RefreshOAuthAccount {
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            document,
        })
        .await
    }

    pub async fn delete_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::DeleteOAuthAccount {
                id,
                expected_config_version,
            },
        )
        .await
    }
}
