use any2api_domain::ConfigRevision;
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
    vault::SecretVault,
};

use super::StoredConfiguration;

pub(crate) async fn load_configuration_from(
    connection: &mut SqliteConnection,
    vault: &SecretVault,
) -> Result<StoredConfiguration, StorageError> {
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(&mut *connection)
            .await?;

    let revision = parse_revision(revision)?;
    let (proxies, proxy_passwords) = load_proxies_from(connection, vault).await?;
    let provider_endpoints = load_provider_endpoints_from(connection).await?;
    let model_routes = load_model_routes_from(connection, &provider_endpoints).await?;
    let (provider_credentials, provider_credential_secrets) =
        load_provider_credentials_from(connection, vault, &provider_endpoints, &proxies).await?;
    let (oauth_accounts, oauth_account_materials) =
        load_oauth_accounts_from(connection, &proxies).await?;
    let gateway_api_key_verifier = vault.gateway_api_key_verifier();
    let gateway_api_keys =
        load_gateway_api_keys_from(connection, &gateway_api_key_verifier).await?;
    let settings = load_settings_from(connection).await?;

    Ok(StoredConfiguration::new(
        revision,
        proxies,
        provider_endpoints,
        provider_credentials,
        oauth_accounts,
        model_routes,
        gateway_api_keys,
        gateway_api_key_verifier,
        settings,
        provider_credential_secrets,
        oauth_account_materials,
        proxy_passwords,
    ))
}

fn parse_revision(value: i64) -> Result<ConfigRevision, StorageError> {
    let revision = u64::try_from(value).map_err(|_| StorageError::InvalidRevision(value))?;
    ConfigRevision::new(revision).map_err(|_| StorageError::InvalidRevision(value))
}
