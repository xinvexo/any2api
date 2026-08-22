use any2api_domain::{
    ModelRouteConfiguration, OAuthAccountConfiguration, ProviderCredentialConfiguration,
};
use sqlx::SqliteConnection;

use crate::{
    error::StorageError,
    gateway_api_key::{GatewayApiKeyVerifier, load_gateway_api_keys_from},
    oauth_account::load_oauth_accounts_from,
    provider::{
        load_model_routes_from, load_provider_credentials_from, load_provider_endpoints_from,
    },
    proxy::load_proxies_from,
    settings::load_settings_from,
};

use super::{StoredConfiguration, StoredConfigurationParts, revision::load_revision_from};

pub(crate) async fn readback_proxy_mutation(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
) -> Result<StoredConfiguration, StorageError> {
    let mut parts = parts_with_stored_revision(connection, current).await?;
    let (proxies, proxy_passwords) = load_proxies_from(connection).await?;
    parts.core.provider_credentials = ProviderCredentialConfiguration::new(
        parts.core.provider_credentials.credentials().to_vec(),
        &parts.core.provider_endpoints,
        &proxies,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    parts.core.oauth_accounts =
        OAuthAccountConfiguration::new(parts.core.oauth_accounts.accounts().to_vec(), &proxies)
            .map_err(|_| StorageError::CorruptConfiguration)?;
    parts.core.proxies = proxies;
    parts.proxy_passwords = proxy_passwords;
    Ok(StoredConfiguration::from_parts(parts))
}

pub(crate) async fn readback_provider_endpoint_mutation(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
    credential_rows_changed: bool,
    model_routes_changed: bool,
) -> Result<StoredConfiguration, StorageError> {
    let mut parts = parts_with_stored_revision(connection, current).await?;
    let endpoints = load_provider_endpoints_from(connection).await?;
    if credential_rows_changed {
        let (credentials, secrets) =
            load_provider_credentials_from(connection, &endpoints, &parts.core.proxies).await?;
        parts.core.provider_credentials = credentials;
        parts.provider_credential_secrets = secrets;
    } else {
        parts.core.provider_credentials = ProviderCredentialConfiguration::new(
            parts.core.provider_credentials.credentials().to_vec(),
            &endpoints,
            &parts.core.proxies,
        )
        .map_err(|_| StorageError::CorruptConfiguration)?;
    }
    parts.core.model_routes = if model_routes_changed {
        load_model_routes_from(connection, &endpoints).await?
    } else {
        ModelRouteConfiguration::new(parts.core.model_routes.routes().to_vec(), &endpoints)
            .map_err(|_| StorageError::CorruptConfiguration)?
    };
    if model_routes_changed {
        parts.core.settings = load_settings_from(connection).await?;
    }
    parts.core.provider_endpoints = endpoints;
    Ok(StoredConfiguration::from_parts(parts))
}

pub(crate) async fn readback_provider_credential_mutation(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
    model_routes_changed: bool,
) -> Result<StoredConfiguration, StorageError> {
    let mut parts = parts_with_stored_revision(connection, current).await?;
    let (credentials, secrets) = load_provider_credentials_from(
        connection,
        &parts.core.provider_endpoints,
        &parts.core.proxies,
    )
    .await?;
    parts.core.provider_credentials = credentials;
    parts.provider_credential_secrets = secrets;
    if model_routes_changed {
        parts.core.model_routes =
            load_model_routes_from(connection, &parts.core.provider_endpoints).await?;
        parts.core.settings = load_settings_from(connection).await?;
    }
    Ok(StoredConfiguration::from_parts(parts))
}

pub(crate) async fn readback_oauth_account_mutation(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
    settings_may_change: bool,
) -> Result<StoredConfiguration, StorageError> {
    let mut parts = parts_with_stored_revision(connection, current).await?;
    let (accounts, materials) = load_oauth_accounts_from(connection, &parts.core.proxies).await?;
    parts.core.oauth_accounts = accounts;
    parts.oauth_account_materials = materials;
    if settings_may_change {
        parts.core.settings = load_settings_from(connection).await?;
    }
    Ok(StoredConfiguration::from_parts(parts))
}

pub(crate) async fn readback_gateway_api_key_mutation(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
) -> Result<StoredConfiguration, StorageError> {
    let mut parts = parts_with_stored_revision(connection, current).await?;
    let verifier = GatewayApiKeyVerifier::new();
    parts.core.gateway_api_keys = load_gateway_api_keys_from(connection, &verifier).await?;
    Ok(StoredConfiguration::from_parts(parts))
}

pub(crate) async fn readback_setting_mutation(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
) -> Result<StoredConfiguration, StorageError> {
    let mut parts = parts_with_stored_revision(connection, current).await?;
    parts.core.settings = load_settings_from(connection).await?;
    Ok(StoredConfiguration::from_parts(parts))
}

async fn parts_with_stored_revision(
    connection: &mut SqliteConnection,
    current: StoredConfiguration,
) -> Result<StoredConfigurationParts, StorageError> {
    let mut parts = current.into_parts();
    parts.core.revision = load_revision_from(connection).await?;
    Ok(parts)
}
