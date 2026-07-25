use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_provider::api::{OAuthTokenMaterial, ProviderRegistry};

use crate::configuration::ConfigPublisher;

use crate::oauth::{document, error::OAuthError};

use super::types::OAuthActivationResult;

pub(in crate::oauth) async fn publish(
    providers: &ProviderRegistry,
    publisher: &ConfigPublisher,
    provider: ProviderKind,
    token: OAuthTokenMaterial,
) -> Result<OAuthActivationResult, OAuthError> {
    if token.provider() != provider {
        return Err(OAuthError::TokenResponseInvalid);
    }
    let driver = providers
        .get(provider)
        .ok_or(OAuthError::ProviderUnavailable)?;
    let routing_profile = driver.oauth_routing_profile(&token)?;
    let models = routing_profile
        .models()
        .iter()
        .map(|model| model.as_str().to_owned())
        .collect();
    let document = document::serialize(&token)?;
    let account_id = OAuthAccountId::new();
    let draft = OAuthAccountDraft::new(default_label(token.email(), account_id), None, true)
        .map_err(|_| OAuthError::DocumentSerialization)?;
    let published = publisher
        .activate_oauth_account(
            account_id,
            provider,
            draft,
            token.email().map(str::to_owned),
            token.expires_at(),
            models,
            document,
        )
        .await
        .map_err(OAuthError::Activation)?;
    let account = published
        .oauth_accounts()
        .get(account_id)
        .cloned()
        .expect("published OAuth account is present after activation");
    Ok(OAuthActivationResult::new(published.revision(), account))
}

fn default_label(email: Option<&str>, account_id: OAuthAccountId) -> String {
    email
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| account_id.to_string())
}
