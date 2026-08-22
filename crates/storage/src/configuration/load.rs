use any2api_domain::{ConfigurationCore, GatewayApiKeyVerifier};
use sqlx::SqliteConnection;

use crate::{
    error::StorageError,
    gateway_api_key::load_gateway_api_keys_from,
    oauth_account::load_oauth_accounts_from,
    provider::{
        load_model_routes_from, load_provider_credentials_from, load_provider_endpoints_from,
    },
    proxy::load_proxies_from,
    settings::load_settings_from,
};

use super::{StoredConfiguration, load_revision_from};

pub(crate) async fn load_configuration_from(
    connection: &mut SqliteConnection,
) -> Result<StoredConfiguration, StorageError> {
    let revision = load_revision_from(connection).await?;
    let (proxies, proxy_passwords) = load_proxies_from(connection).await?;
    let provider_endpoints = load_provider_endpoints_from(connection).await?;
    let model_routes = load_model_routes_from(connection, &provider_endpoints).await?;
    let (provider_credentials, provider_credential_secrets) =
        load_provider_credentials_from(connection, &provider_endpoints, &proxies).await?;
    let (oauth_accounts, oauth_account_materials) =
        load_oauth_accounts_from(connection, &proxies).await?;
    let gateway_api_key_verifier = GatewayApiKeyVerifier::new();
    let gateway_api_keys =
        load_gateway_api_keys_from(connection, &gateway_api_key_verifier).await?;
    let settings = load_settings_from(connection).await?;

    Ok(StoredConfiguration::new(
        ConfigurationCore::new(
            revision,
            proxies,
            provider_endpoints,
            provider_credentials,
            oauth_accounts,
            model_routes,
            gateway_api_keys,
            settings,
        ),
        provider_credential_secrets,
        oauth_account_materials,
        proxy_passwords,
    ))
}
