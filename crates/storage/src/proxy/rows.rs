use std::{collections::HashMap, str::FromStr};

use any2api_domain::{
    ProxyAddress, ProxyAuthentication, ProxyConfiguration, ProxyKind, ProxyProfile, ProxyProfileId,
};
use sqlx::{FromRow, SqliteConnection};

use crate::{error::StorageError, secret::SecretBytes};

use super::{
    password::validate as validate_proxy_password,
    password_material::{StoredProxyPassword, StoredProxyPasswords},
};

#[derive(Debug, FromRow)]
struct ProxyRow {
    id: String,
    name: String,
    kind: String,
    host: Option<String>,
    port: Option<i64>,
    enabled: i64,
    authentication_version: i64,
    config_version: i64,
}

#[derive(FromRow)]
struct ProxyPasswordRow {
    proxy_profile_id: String,
    username: String,
    authentication_version: i64,
    password: Vec<u8>,
}

pub(crate) async fn load_proxies_from(
    connection: &mut SqliteConnection,
) -> Result<(ProxyConfiguration, StoredProxyPasswords), StorageError> {
    let global_id: String = sqlx::query_scalar(
        "SELECT global_proxy_profile_id FROM proxy_settings WHERE singleton_id = 1",
    )
    .fetch_one(&mut *connection)
    .await?;
    let rows = sqlx::query_as::<_, ProxyRow>(
        "SELECT id, name, kind, host, port, enabled, authentication_version, config_version \
         FROM proxy_profiles ORDER BY built_in DESC, name ASC",
    )
    .fetch_all(&mut *connection)
    .await?;
    let password_rows = sqlx::query_as::<_, ProxyPasswordRow>(
        "SELECT proxy_profile_id, username, authentication_version, password FROM proxy_passwords",
    )
    .fetch_all(&mut *connection)
    .await?;

    let global_id =
        ProxyProfileId::from_str(&global_id).map_err(|_| StorageError::CorruptConfiguration)?;
    let mut password_rows = password_rows_by_id(password_rows)?;
    let mut profiles = Vec::with_capacity(rows.len());
    let mut proxy_passwords = Vec::with_capacity(password_rows.len());
    for row in rows {
        let id =
            ProxyProfileId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
        let password_row = password_rows.remove(&id);
        let (profile, password) = parse_profile(row, password_row)?;
        profiles.push(profile);
        if let Some(password) = password {
            proxy_passwords.push(password);
        }
    }
    if !password_rows.is_empty() {
        return Err(StorageError::CorruptConfiguration);
    }
    let proxies = ProxyConfiguration::new(profiles, global_id)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    Ok((proxies, StoredProxyPasswords::new(proxy_passwords)))
}

fn parse_profile(
    row: ProxyRow,
    password_row: Option<ProxyPasswordRow>,
) -> Result<(ProxyProfile, Option<StoredProxyPassword>), StorageError> {
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
    let authentication_version = u64::try_from(row.authentication_version)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let authentication = password_row
        .as_ref()
        .map(|row| ProxyAuthentication::new(row.username.clone()))
        .transpose()
        .map_err(|_| StorageError::CorruptConfiguration)?;

    let profile = ProxyProfile::restore(
        id,
        row.name,
        kind,
        address,
        row.enabled == 1,
        authentication,
        authentication_version,
        version,
    )
    .map_err(|_| StorageError::CorruptConfiguration)?;
    let password = password_row
        .map(|row| parse_proxy_password(row, &profile))
        .transpose()?;
    Ok((profile, password))
}

fn password_rows_by_id(
    rows: Vec<ProxyPasswordRow>,
) -> Result<HashMap<ProxyProfileId, ProxyPasswordRow>, StorageError> {
    let mut by_id = HashMap::with_capacity(rows.len());
    for row in rows {
        let id = ProxyProfileId::from_str(&row.proxy_profile_id)
            .map_err(|_| StorageError::CorruptConfiguration)?;
        if by_id.insert(id, row).is_some() {
            return Err(StorageError::CorruptConfiguration);
        }
    }
    Ok(by_id)
}

fn parse_proxy_password(
    row: ProxyPasswordRow,
    profile: &ProxyProfile,
) -> Result<StoredProxyPassword, StorageError> {
    let authentication_version = u64::try_from(row.authentication_version)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    if authentication_version != profile.authentication_version()
        || profile.authentication().is_none()
    {
        return Err(StorageError::CorruptConfiguration);
    }
    let secret: SecretBytes = row.password.into();
    validate_proxy_password(&secret)?;
    Ok(StoredProxyPassword::new(
        profile.id(),
        authentication_version,
        secret,
    ))
}

fn parse_kind(value: &str) -> Result<ProxyKind, StorageError> {
    match value {
        "direct" => Ok(ProxyKind::Direct),
        "http" => Ok(ProxyKind::Http),
        "socks5" => Ok(ProxyKind::Socks5),
        _ => Err(StorageError::CorruptConfiguration),
    }
}
