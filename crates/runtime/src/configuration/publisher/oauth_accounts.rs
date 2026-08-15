use std::{collections::HashSet, sync::Arc};

use any2api_domain::{
    ConfigRevision, OAuthAccountDraft, OAuthAccountId, OAuthProxySelection, ProviderKind,
};
use any2api_provider::api::OAuthTokenMaterial;
use any2api_storage::api::{
    ConfigurationMutation, OAuthAccountCreate, OAuthAccountDocument, OAuthAccountRefresh,
};

use super::ConfigPublisher;
use crate::configuration::{
    ConfigPublishError, OAuthImportIdentity, OAuthImportIdentityIndex, PublicationSource,
    PublishedSnapshot, command::ConfigCommand, publish_task,
};

pub(crate) struct OAuthAccountActivation {
    pub(crate) id: OAuthAccountId,
    pub(crate) provider_kind: ProviderKind,
    pub(crate) proxy_selection: OAuthProxySelection,
    pub(crate) preferred_label: Option<String>,
    pub(crate) safe_account_email: Option<String>,
    pub(crate) expires_at: Option<i64>,
    pub(crate) models: Vec<String>,
    pub(crate) document: OAuthAccountDocument,
    pub(crate) token: OAuthTokenMaterial,
}

impl ConfigPublisher {
    pub(super) fn validate_oauth_account_models(
        &self,
        current: &PublishedSnapshot,
        id: OAuthAccountId,
        _models: &[String],
    ) -> Result<(), ConfigPublishError> {
        current
            .oauth_accounts()
            .get(id)
            .ok_or(ConfigPublishError::OAuthAccountNotFound)
            .map(|_| ())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn activate_oauth_account(
        &self,
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish_current(
            PublicationSource::Operator,
            ConfigCommand::CreateOAuthAccount {
                id,
                provider_kind,
                draft,
                proxy_selection,
                safe_account_email,
                expires_at,
                models,
                document,
            },
        )
        .await
    }

    pub(crate) async fn activate_oauth_accounts(
        &self,
        activations: Vec<OAuthAccountActivation>,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher
                .activate_oauth_accounts_serialized(activations)
                .await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn activate_oauth_accounts_serialized(
        &self,
        activations: Vec<OAuthAccountActivation>,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let expected = current.revision();
        validate_import_identities(
            current.as_ref(),
            self.capabilities.provider_registry(),
            &activations,
        )?;
        let mut labels = current
            .oauth_accounts()
            .accounts()
            .iter()
            .map(|account| (account.provider_kind(), account.label_key()))
            .collect::<HashSet<_>>();
        let mut creates = Vec::with_capacity(activations.len());
        for activation in activations {
            let label = unique_label(
                &mut labels,
                activation.provider_kind,
                activation.preferred_label.as_deref(),
                activation.id,
            );
            let draft = OAuthAccountDraft::new(label, None, true)
                .map_err(ConfigPublishError::InvalidOAuthAccount)?;
            creates.push(OAuthAccountCreate::new(
                activation.id,
                activation.provider_kind,
                draft,
                activation.proxy_selection,
                activation.safe_account_email,
                activation.expires_at,
                activation.models,
                activation.document,
            ));
        }
        let (published, _) = self
            .publish_mutation_serialized(
                current,
                expected,
                ConfigurationMutation::CreateOAuthAccounts { accounts: creates },
            )
            .await?;
        Ok(published)
    }

    pub async fn update_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
    ) -> Result<Arc<PublishedSnapshot>, ConfigPublishError> {
        self.publish(
            expected,
            ConfigCommand::UpdateOAuthAccount {
                id,
                expected_config_version,
                draft,
                proxy_selection,
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
        self.publish_current(
            PublicationSource::AutomaticOAuthRefresh,
            ConfigCommand::RefreshOAuthAccount {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                document,
            },
        )
        .await
    }

    pub(crate) async fn refresh_oauth_accounts<K>(
        &self,
        refreshes: Vec<OAuthAccountRefresh>,
        publication_keepalive: K,
    ) -> Result<(Arc<PublishedSnapshot>, bool), ConfigPublishError>
    where
        K: Send + 'static,
    {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            let result = publisher.refresh_oauth_accounts_serialized(refreshes).await;
            drop(publication_keepalive);
            result
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn refresh_oauth_accounts_serialized(
        &self,
        refreshes: Vec<OAuthAccountRefresh>,
    ) -> Result<(Arc<PublishedSnapshot>, bool), ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let expected = current.revision();
        self.publish_mutation_serialized_with_source(
            current,
            expected,
            ConfigurationMutation::RefreshOAuthAccounts { refreshes },
            PublicationSource::AutomaticOAuthRefresh,
        )
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

fn validate_import_identities(
    current: &PublishedSnapshot,
    providers: &any2api_provider::api::ProviderRegistry,
    activations: &[OAuthAccountActivation],
) -> Result<(), ConfigPublishError> {
    let mut identities = OAuthImportIdentityIndex::default();
    for account in current.oauth_accounts().accounts() {
        if let Some(token) = current.oauth_token_material(account.id()) {
            let driver = providers.get(token.provider()).ok_or(
                crate::configuration::ConfigurationCapabilityError::MissingProviderDriver(
                    token.provider(),
                ),
            )?;
            identities.include_existing(&OAuthImportIdentity::from_token(
                driver.as_ref(),
                token.as_ref(),
            ));
        }
    }
    for activation in activations {
        let driver = providers.get(activation.provider_kind).ok_or(
            crate::configuration::ConfigurationCapabilityError::MissingProviderDriver(
                activation.provider_kind,
            ),
        )?;
        let identity = OAuthImportIdentity::from_token(driver.as_ref(), &activation.token);
        if !identities.insert_new(&identity) {
            return Err(ConfigPublishError::OAuthAccountIdentityConflict);
        }
    }
    Ok(())
}

pub(super) fn unique_label(
    used: &mut HashSet<(ProviderKind, String)>,
    provider: ProviderKind,
    preferred: Option<&str>,
    id: OAuthAccountId,
) -> String {
    const MAX_LABEL_CHARS: usize = 100;
    let mut base = preferred
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_control())
        .collect::<String>();
    base = base
        .trim()
        .chars()
        .take(MAX_LABEL_CHARS)
        .collect::<String>()
        .trim_end()
        .to_owned();
    if base.is_empty() {
        base = id.to_string();
    }
    for sequence in 1_u32.. {
        let candidate = if sequence == 1 {
            base.clone()
        } else {
            let suffix = format!(" ({sequence})");
            let prefix_chars = MAX_LABEL_CHARS.saturating_sub(suffix.chars().count());
            format!(
                "{}{}",
                base.chars().take(prefix_chars).collect::<String>(),
                suffix
            )
        };
        if used.insert((provider, candidate.to_lowercase())) {
            return candidate;
        }
    }
    unreachable!("OAuth label sequence is unbounded")
}

#[cfg(test)]
#[path = "oauth_accounts_tests.rs"]
mod tests;
