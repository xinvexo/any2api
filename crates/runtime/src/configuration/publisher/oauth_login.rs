use std::{collections::HashSet, sync::Arc};

use any2api_domain::{OAuthAccountDraft, OAuthAccountId};
use any2api_provider::api::{OAuthTokenMaterial, encode_oauth_account_document};
use any2api_storage::api::{ConfigurationMutation, OAuthAccountDocument};

use super::{ConfigPublisher, OAuthAccountActivation, oauth_accounts::unique_label};
use crate::configuration::{ConfigPublishError, PublishedSnapshot, publish_task};

impl ConfigPublisher {
    pub(crate) async fn activate_oauth_login(
        &self,
        activation: OAuthAccountActivation,
    ) -> Result<(Arc<PublishedSnapshot>, OAuthAccountId), ConfigPublishError> {
        let publisher = self.clone();
        publish_task::run(self.runtime.lifecycle(), async move {
            publisher.activate_oauth_login_serialized(activation).await
        })
        .await
        .ok_or(ConfigPublishError::ShuttingDown)?
    }

    async fn activate_oauth_login_serialized(
        &self,
        activation: OAuthAccountActivation,
    ) -> Result<(Arc<PublishedSnapshot>, OAuthAccountId), ConfigPublishError> {
        let _guard = self.snapshots.acquire_publish().await;
        let current = self.snapshots.load();
        let expected = current.revision();
        let (account_id, mutation) = login_mutation(current.as_ref(), activation)?;
        let (published, _) = self
            .publish_mutation_serialized(current, expected, mutation)
            .await?;
        Ok((published, account_id))
    }
}

fn login_mutation(
    current: &PublishedSnapshot,
    activation: OAuthAccountActivation,
) -> Result<(OAuthAccountId, ConfigurationMutation), ConfigPublishError> {
    let identity = crate::configuration::OAuthImportIdentity::from_token(&activation.token);
    let matches = current
        .oauth_accounts()
        .accounts()
        .iter()
        .filter_map(|account| {
            let token = current.oauth_token_material(account.id())?;
            Some((
                account.id(),
                identity.stable_matches(token.as_ref()),
                identity.exact_token_matches(token.as_ref()),
            ))
        });
    match resolve_login_match(matches)? {
        Some(account_id) => reauthorize_mutation(current, account_id, activation),
        None => create_mutation(current, activation),
    }
}

fn resolve_login_match(
    matches: impl IntoIterator<Item = (OAuthAccountId, bool, bool)>,
) -> Result<Option<OAuthAccountId>, ConfigPublishError> {
    let mut stable = HashSet::new();
    let mut exact = HashSet::new();
    for (id, stable_match, exact_match) in matches {
        if stable_match {
            stable.insert(id);
        }
        if exact_match {
            exact.insert(id);
        }
    }
    if stable.len() > 1
        || exact.len() > 1
        || (stable.len() == 1 && !exact.is_empty() && stable != exact)
    {
        return Err(ConfigPublishError::OAuthAccountIdentityConflict);
    }
    Ok(stable
        .into_iter()
        .next()
        .or_else(|| exact.into_iter().next()))
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
    let safe_account_email = activation
        .safe_account_email
        .or_else(|| account.safe_account_email().map(str::to_owned));
    let document = reauthorization_document(current, account_id, &activation.token)?;
    Ok((
        account_id,
        ConfigurationMutation::ReauthorizeOAuthAccount {
            id: account_id,
            expected_token_version: account.token_version(),
            safe_account_email,
            expires_at: activation.expires_at,
            models,
            document,
        },
    ))
}

fn reauthorization_document(
    current: &PublishedSnapshot,
    account_id: OAuthAccountId,
    token: &OAuthTokenMaterial,
) -> Result<OAuthAccountDocument, ConfigPublishError> {
    let Some(previous) = current.oauth_token_material(account_id) else {
        return Err(ConfigPublishError::OAuthAccountNotFound);
    };
    let merged = token
        .clone()
        .merge_missing_identity(previous.as_ref())
        .map_err(|_| ConfigPublishError::OAuthAccountIdentityConflict)?;
    let bytes = encode_oauth_account_document(&merged)
        .map_err(|_| ConfigPublishError::OAuthAccountIdentityConflict)?;
    OAuthAccountDocument::new(merged.provider(), bytes.into())
        .map_err(|_| ConfigPublishError::OAuthAccountIdentityConflict)
}

#[cfg(test)]
mod tests {
    use any2api_domain::OAuthAccountId;

    use super::resolve_login_match;

    #[test]
    fn exact_token_match_can_reuse_one_account_without_stable_identity() {
        let id = OAuthAccountId::new();
        assert_eq!(
            resolve_login_match([(id, false, true)]).expect("match"),
            Some(id)
        );
    }

    #[test]
    fn stable_and_exact_matches_for_the_same_account_reuse_it() {
        let id = OAuthAccountId::new();
        assert_eq!(
            resolve_login_match([(id, true, true)]).expect("match"),
            Some(id)
        );
    }

    #[test]
    fn unproven_login_remains_a_new_account() {
        let id = OAuthAccountId::new();
        assert_eq!(
            resolve_login_match([(id, false, false)]).expect("match"),
            None
        );
    }

    #[test]
    fn stable_and_exact_matches_for_different_accounts_fail_closed() {
        let stable = OAuthAccountId::new();
        let exact = OAuthAccountId::new();
        assert!(resolve_login_match([(stable, true, false), (exact, false, true)]).is_err());
    }

    #[test]
    fn multiple_exact_matches_fail_closed() {
        let first = OAuthAccountId::new();
        let second = OAuthAccountId::new();
        assert!(resolve_login_match([(first, false, true), (second, false, true)]).is_err());
    }
}
