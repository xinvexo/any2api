use any2api_domain::ProxyProfile;
use secrecy::ExposeSecret;
use sqlx::SqliteConnection;

use crate::{error::StorageError, secret::SecretBytes};

pub(crate) async fn set_authentication(
    connection: &mut SqliteConnection,
    profile: &ProxyProfile,
    password: &SecretBytes,
) -> Result<(), StorageError> {
    let authentication = profile
        .authentication()
        .ok_or(StorageError::CorruptConfiguration)?;
    update_profile_versions(connection, profile).await?;
    sqlx::query(
        "INSERT INTO proxy_passwords \
         (proxy_profile_id, username, authentication_version, password) \
         VALUES (?, ?, ?, ?) \
         ON CONFLICT(proxy_profile_id) DO UPDATE SET \
          username = excluded.username, authentication_version = excluded.authentication_version, \
          password = excluded.password, \
          updated_at = CURRENT_TIMESTAMP",
    )
    .bind(profile.id().to_string())
    .bind(authentication.username())
    .bind(to_i64(profile.authentication_version())?)
    .bind(password.expose_secret())
    .execute(connection)
    .await?;
    Ok(())
}

pub(crate) async fn clear_authentication(
    connection: &mut SqliteConnection,
    profile: &ProxyProfile,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM proxy_passwords WHERE proxy_profile_id = ?")
        .bind(profile.id().to_string())
        .execute(&mut *connection)
        .await?;
    update_profile_versions(connection, profile).await
}

async fn update_profile_versions(
    connection: &mut SqliteConnection,
    profile: &ProxyProfile,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE proxy_profiles SET authentication_version = ?, config_version = ?, \
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(to_i64(profile.authentication_version())?)
    .bind(to_i64(profile.config_version())?)
    .bind(profile.id().to_string())
    .execute(connection)
    .await?;
    if result.rows_affected() != 1 {
        return Err(StorageError::ProxyNotFound(profile.id()));
    }
    Ok(())
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::RevisionOverflow)
}
