use std::str::FromStr;

use any2api_domain::{
    ConfigRevision, ProxyAddress, ProxyConfiguration, ProxyKind, ProxyProfile, ProxyProfileId,
};
use sqlx::{FromRow, SqliteConnection};

use crate::{
    configuration::StoredConfiguration, error::StorageError,
    gateway_api_key_rows::load_gateway_api_keys_from, model_route_rows::load_model_routes_from,
    provider_credential_rows::load_provider_credentials_from,
    provider_endpoint_rows::load_provider_endpoints_from, proxy_mutation::DatabaseChange,
    settings_rows::load_settings_from, vault::SecretVault,
};

#[derive(Debug, FromRow)]
struct ProxyRow {
    id: String,
    name: String,
    kind: String,
    host: Option<String>,
    port: Option<i64>,
    enabled: i64,
    config_version: i64,
}

pub(crate) async fn load_configuration_from(
    connection: &mut SqliteConnection,
    vault: &SecretVault,
) -> Result<StoredConfiguration, StorageError> {
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(&mut *connection)
            .await?;
    let global_id: String = sqlx::query_scalar(
        "SELECT global_proxy_profile_id FROM proxy_settings WHERE singleton_id = 1",
    )
    .fetch_one(&mut *connection)
    .await?;
    let rows = sqlx::query_as::<_, ProxyRow>(
        "SELECT id, name, kind, host, port, enabled, config_version \
         FROM proxy_profiles ORDER BY built_in DESC, name ASC",
    )
    .fetch_all(&mut *connection)
    .await?;

    let revision = parse_revision(revision)?;
    let global_id =
        ProxyProfileId::from_str(&global_id).map_err(|_| StorageError::CorruptConfiguration)?;
    let profiles = rows
        .into_iter()
        .map(parse_profile)
        .collect::<Result<Vec<_>, _>>()?;
    let proxies = ProxyConfiguration::new(profiles, global_id)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let provider_endpoints = load_provider_endpoints_from(connection).await?;
    let model_routes = load_model_routes_from(connection, &provider_endpoints).await?;
    let (provider_credentials, provider_credential_secrets) =
        load_provider_credentials_from(connection, vault, &provider_endpoints, &proxies).await?;
    let gateway_api_key_verifier = vault.gateway_api_key_verifier();
    let gateway_api_keys =
        load_gateway_api_keys_from(connection, &gateway_api_key_verifier).await?;
    let settings = load_settings_from(connection).await?;

    Ok(StoredConfiguration::new(
        revision,
        proxies,
        provider_endpoints,
        provider_credentials,
        model_routes,
        gateway_api_keys,
        gateway_api_key_verifier,
        settings,
        provider_credential_secrets,
    ))
}

pub(crate) async fn execute_change(
    connection: &mut SqliteConnection,
    change: &DatabaseChange,
) -> Result<(), StorageError> {
    match change {
        DatabaseChange::Create(profile) => insert_profile(connection, profile).await?,
        DatabaseChange::Update(profile) => update_profile(connection, profile).await?,
        DatabaseChange::Delete(id) => delete_profile(connection, *id).await?,
        DatabaseChange::SetGlobal(id) => set_global(connection, *id).await?,
    }

    Ok(())
}

async fn insert_profile(
    connection: &mut SqliteConnection,
    profile: &ProxyProfile,
) -> Result<(), StorageError> {
    let address = profile
        .address()
        .ok_or(StorageError::CorruptConfiguration)?;
    sqlx::query(
        "INSERT INTO proxy_profiles \
         (id, name, name_key, kind, host, port, enabled, built_in, config_version) \
         VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(profile.id().to_string())
    .bind(profile.name())
    .bind(profile.name_key())
    .bind(kind_text(profile.kind()))
    .bind(address.host())
    .bind(i64::from(address.port()))
    .bind(profile.enabled())
    .bind(i64::try_from(profile.config_version()).map_err(|_| StorageError::RevisionOverflow)?)
    .execute(connection)
    .await?;
    Ok(())
}

async fn update_profile(
    connection: &mut SqliteConnection,
    profile: &ProxyProfile,
) -> Result<(), StorageError> {
    let address = profile
        .address()
        .ok_or(StorageError::CorruptConfiguration)?;
    let result = sqlx::query(
        "UPDATE proxy_profiles SET \
         name = ?, name_key = ?, kind = ?, host = ?, port = ?, enabled = ?, \
         config_version = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(profile.name())
    .bind(profile.name_key())
    .bind(kind_text(profile.kind()))
    .bind(address.host())
    .bind(i64::from(address.port()))
    .bind(profile.enabled())
    .bind(i64::try_from(profile.config_version()).map_err(|_| StorageError::RevisionOverflow)?)
    .bind(profile.id().to_string())
    .execute(connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ProxyNotFound(profile.id()));
    }
    Ok(())
}

async fn delete_profile(
    connection: &mut SqliteConnection,
    id: ProxyProfileId,
) -> Result<(), StorageError> {
    let result = sqlx::query("DELETE FROM proxy_profiles WHERE id = ?")
        .bind(id.to_string())
        .execute(connection)
        .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ProxyNotFound(id));
    }
    Ok(())
}

async fn set_global(
    connection: &mut SqliteConnection,
    id: ProxyProfileId,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE proxy_settings SET global_proxy_profile_id = ?, updated_at = CURRENT_TIMESTAMP \
         WHERE singleton_id = 1",
    )
    .bind(id.to_string())
    .execute(connection)
    .await?;
    Ok(())
}

fn parse_profile(row: ProxyRow) -> Result<ProxyProfile, StorageError> {
    let id = ProxyProfileId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
    let kind = parse_kind(&row.kind)?;
    let address = match (row.host, row.port) {
        (Some(host), Some(port)) => {
            let port = u16::try_from(port).map_err(|_| StorageError::CorruptConfiguration)?;
            Some(ProxyAddress::new(host, port).map_err(|_| StorageError::CorruptConfiguration)?)
        }
        (None, None) => None,
        _ => return Err(StorageError::CorruptConfiguration),
    };
    let version =
        u64::try_from(row.config_version).map_err(|_| StorageError::CorruptConfiguration)?;

    ProxyProfile::restore(id, row.name, kind, address, row.enabled == 1, version)
        .map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_revision(value: i64) -> Result<ConfigRevision, StorageError> {
    let revision = u64::try_from(value).map_err(|_| StorageError::InvalidRevision(value))?;
    ConfigRevision::new(revision).map_err(|_| StorageError::InvalidRevision(value))
}

fn parse_kind(value: &str) -> Result<ProxyKind, StorageError> {
    match value {
        "direct" => Ok(ProxyKind::Direct),
        "http" => Ok(ProxyKind::Http),
        "socks5" => Ok(ProxyKind::Socks5),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

const fn kind_text(kind: ProxyKind) -> &'static str {
    match kind {
        ProxyKind::Direct => "direct",
        ProxyKind::Http => "http",
        ProxyKind::Socks5 => "socks5",
    }
}
