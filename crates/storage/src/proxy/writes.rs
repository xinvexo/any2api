use any2api_domain::{ProxyKind, ProxyProfile, ProxyProfileId};
use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::mutation::DatabaseChange;

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
         authentication_version = ?, config_version = ?, updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?",
    )
    .bind(profile.name())
    .bind(profile.name_key())
    .bind(kind_text(profile.kind()))
    .bind(address.host())
    .bind(i64::from(address.port()))
    .bind(profile.enabled())
    .bind(
        i64::try_from(profile.authentication_version())
            .map_err(|_| StorageError::RevisionOverflow)?,
    )
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

const fn kind_text(kind: ProxyKind) -> &'static str {
    match kind {
        ProxyKind::Direct => "direct",
        ProxyKind::Http => "http",
        ProxyKind::Socks5 => "socks5",
    }
}
