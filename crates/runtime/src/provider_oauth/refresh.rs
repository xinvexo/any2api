use std::{sync::Arc, time::Duration};

use any2api_domain::{CredentialId, CredentialKind};
use any2api_provider::api::{OAuthGrant, OAuthTokenMaterial};

use crate::provider_oauth2_secret::ProviderOAuth2Secret;

use super::{error::ProviderOAuthError, service::ProviderOAuthService, token_request};

const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const REFRESH_LEAD_SECONDS: i64 = 300;

pub(super) async fn run(service: Arc<ProviderOAuthService>) {
    let mut interval = tokio::time::interval(REFRESH_INTERVAL);
    loop {
        interval.tick().await;
        for credential_id in due_candidates(service.as_ref()) {
            if let Err(error) = refresh_one(service.as_ref(), credential_id).await {
                tracing::warn!(credential_id = %credential_id, %error, "provider OAuth token refresh failed");
            }
        }
    }
}

fn due_candidates(service: &ProviderOAuthService) -> Vec<CredentialId> {
    service
        .snapshots
        .load()
        .provider_credentials()
        .credentials()
        .iter()
        .filter(|credential| {
            credential.enabled() && credential.credential_kind() == CredentialKind::OAuth2
        })
        .map(|credential| credential.id())
        .collect()
}

pub(super) async fn refresh_one(
    service: &ProviderOAuthService,
    credential_id: CredentialId,
) -> Result<(), ProviderOAuthError> {
    let snapshot = service.snapshots.load();
    let credential = snapshot
        .provider_credentials()
        .get(credential_id)
        .ok_or(ProviderOAuthError::ConfigurationChanged)?;
    if !credential.enabled() || credential.credential_kind() != CredentialKind::OAuth2 {
        return Ok(());
    }
    let endpoint = snapshot
        .provider_endpoints()
        .get(credential.provider_endpoint_id())
        .ok_or(ProviderOAuthError::ConfigurationChanged)?;
    let driver = service
        .providers
        .get(endpoint.provider_kind())
        .ok_or(ProviderOAuthError::ProviderUnavailable)?;
    let binding = snapshot
        .credential_runtime(credential_id)
        .ok_or(ProviderOAuthError::ConfigurationChanged)?;
    let current = OAuthTokenMaterial::from_secret(
        endpoint.provider_kind(),
        binding.generation().provider_secret(),
    )?;
    if !current.is_expired_or_near_expiry(unix_now(), REFRESH_LEAD_SECONDS) {
        return Ok(());
    }
    let Some(refresh_token) = current.refresh_token() else {
        return Ok(());
    };
    let plan = driver.oauth_token_request(OAuthGrant::RefreshToken, refresh_token, None, None)?;
    let body = token_request::execute(
        service.transport.as_ref(),
        snapshot.as_ref(),
        credential.proxy_profile_id(),
        plan,
    )
    .await?;
    let refreshed = driver
        .parse_oauth_token(&body, Some(&current))
        .map_err(ProviderOAuthError::from_token_response_error)?;
    let oauth_secret = ProviderOAuth2Secret::from_token(&refreshed)?;
    if let Err(error) = service
        .publisher
        .refresh_provider_oauth_credential_secret(
            credential_id,
            credential.secret_version(),
            oauth_secret,
        )
        .await
    {
        binding.generation().health().mark_auth_error();
        return Err(error.into());
    }
    tracing::info!(credential_id = %credential_id, "provider OAuth token refreshed");
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}
