use std::{collections::HashSet, sync::Arc};

use any2api_domain::{OAuthAccountDraft, OAuthAccountId};
use any2api_storage::api::ConfigurationMutation;

use super::{ConfigPublisher, OAuthAccountActivation, oauth_accounts::unique_label};
use crate::configuration::{
    ConfigPublishError, OAuthAccountIdentity, PublishedSnapshot, publish_task,
};

impl ConfigPublisher {
    pub(crate) async fn activate_oauth_login(
        &self,
        activation: OAuthAccountActivation,
        identity: Option<OAuthAccountIdentity>,
    ) -> Result<(Arc<PublishedSnapshot>, OAuthAccountId), ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher
                .activate_oauth_login_serialized(activation, identity.as_ref())
                .await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn activate_oauth_login_serialized(
        &self,
        activation: OAuthAccountActivation,
        identity: Option<&OAuthAccountIdentity>,
    ) -> Result<(Arc<PublishedSnapshot>, OAuthAccountId), ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let expected = current.revision();
        let (account_id, mutation) = login_mutation(current.as_ref(), activation, identity)?;
        let (published, _) = self
            .publish_mutation_serialized(current, expected, mutation)
            .await?;
        Ok((published, account_id))
    }
}

fn login_mutation(
    current: &PublishedSnapshot,
    activation: OAuthAccountActivation,
    identity: Option<&OAuthAccountIdentity>,
) -> Result<(OAuthAccountId, ConfigurationMutation), ConfigPublishError> {
    let matches = identity.map_or_else(Vec::new, |identity| {
        current
            .oauth_accounts()
            .accounts()
            .iter()
            .filter(|account| {
                current
                    .oauth_token_material(account.id())
                    .is_some_and(|token| identity.matches(token.as_ref()))
            })
            .map(|account| account.id())
            .collect::<Vec<_>>()
    });

    match matches.as_slice() {
        [] => create_mutation(current, activation),
        [account_id] => reauthorize_mutation(current, *account_id, activation),
        _ => Err(ConfigPublishError::OAuthAccountIdentityConflict),
    }
}

fn create_mutation(
    current: &PublishedSnapshot,
    activation: OAuthAccountActivation,
) -> Result<(OAuthAccountId, ConfigurationMutation), ConfigPublishError> {
    let mut labels = current
        .oauth_accounts()
        .accounts()
        .iter()
        .map(|account| (account.provider_kind(), account.label_key()))
        .collect::<HashSet<_>>();
    let label = unique_label(
        &mut labels,
        activation.provider_kind,
        activation.preferred_label.as_deref(),
        activation.id,
    );
    let draft = OAuthAccountDraft::new(label, None, true)
        .map_err(ConfigPublishError::InvalidOAuthAccount)?;
    let account_id = activation.id;
    Ok((
        account_id,
        ConfigurationMutation::CreateOAuthAccount {
            id: account_id,
            provider_kind: activation.provider_kind,
            draft,
            safe_account_email: activation.safe_account_email,
            expires_at: activation.expires_at,
            models: activation.models,
            document: activation.document,
        },
    ))
}

fn reauthorize_mutation(
    current: &PublishedSnapshot,
    account_id: OAuthAccountId,
    activation: OAuthAccountActivation,
) -> Result<(OAuthAccountId, ConfigurationMutation), ConfigPublishError> {
    let account = current
        .oauth_accounts()
        .get(account_id)
        .ok_or(ConfigPublishError::OAuthAccountNotFound)?;
    let available = activation
        .models
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let models = account
        .models()
        .iter()
        .filter(|model| available.contains(model.as_str()))
        .map(|model| model.as_str().to_owned())
        .collect();
    Ok((
        account_id,
        ConfigurationMutation::ReauthorizeOAuthAccount {
            id: account_id,
            expected_token_version: account.token_version(),
            safe_account_email: activation.safe_account_email,
            expires_at: activation.expires_at,
            models,
            document: activation.document,
        },
    ))
}
