use any2api_domain::{OAuthAccountId, ProviderKind};
use any2api_provider::api::{OAuthTokenMaterial, ProviderRegistry};

use crate::configuration::{ConfigPublisher, OAuthAccountActivation};

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
    let document = document::build_account_document(&token)?;
    let activation = OAuthAccountActivation {
        id: OAuthAccountId::new(),
        provider_kind: provider,
        preferred_label: token.email().map(str::to_owned),
        safe_account_email: token.email().map(str::to_owned),
        expires_at: token.expires_at(),
        models,
        document,
        token,
    };
    let (published, account_id) = publisher
        .activate_oauth_login(activation)
        .await
        .map_err(OAuthError::Activation)?;
    let account = published
        .oauth_accounts()
        .get(account_id)
        .cloned()
        .expect("published OAuth account is present after activation");
    Ok(OAuthActivationResult::new(published.revision(), account))
}
