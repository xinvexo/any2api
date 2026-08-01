use any2api_domain::{CredentialKind, ProviderCredential, ProviderEndpointId};
use secrecy::ExposeSecret;
use sqlx::SqliteConnection;

use crate::{error::StorageError, secret::SecretBytes};

use super::mutation::ProviderCredentialDatabaseChange;

pub(crate) async fn execute_provider_credential_change(
    connection: &mut SqliteConnection,
    change: &ProviderCredentialDatabaseChange,
) -> Result<(), StorageError> {
    match change {
        ProviderCredentialDatabaseChange::Create {
            credential,
            api_key,
        } => insert(connection, credential, api_key).await?,
        ProviderCredentialDatabaseChange::Update(credential) => {
            update_metadata(connection, credential).await?
        }
        ProviderCredentialDatabaseChange::RotateSecret {
            credential,
            api_key,
        } => {
            rotate_secret(connection, credential, api_key).await?;
            replace_models(connection, credential).await?;
        }
        ProviderCredentialDatabaseChange::SetModels(credential) => {
            update_metadata(connection, credential).await?;
            replace_models(connection, credential).await?;
        }
        ProviderCredentialDatabaseChange::Delete(id) => {
            let result = sqlx::query("DELETE FROM provider_credentials WHERE id = ?")
                .bind(id.to_string())
                .execute(connection)
                .await?;
            if result.rows_affected() != 1 {
                return Err(StorageError::ProviderCredentialNotFound(*id));
            }
        }
    }
    Ok(())
}

async fn replace_models(
    connection: &mut SqliteConnection,
    credential: &ProviderCredential,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM provider_credential_models WHERE credential_id = ?")
        .bind(credential.id().to_string())
        .execute(&mut *connection)
        .await?;
    for model in credential.models() {
        sqlx::query(
            "INSERT INTO provider_credential_models (credential_id, upstream_model) VALUES (?, ?)",
        )
        .bind(credential.id().to_string())
        .bind(model.as_str())
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

pub(crate) async fn bump_endpoint_credential_generations(
    connection: &mut SqliteConnection,
    endpoint_id: ProviderEndpointId,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE provider_credentials SET credential_generation = credential_generation + 1, \
         updated_at = CURRENT_TIMESTAMP WHERE provider_endpoint_id = ?",
    )
    .bind(endpoint_id.to_string())
    .execute(connection)
    .await?;
    Ok(())
}

async fn insert(
    connection: &mut SqliteConnection,
    credential: &ProviderCredential,
    api_key: &SecretBytes,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, \
          secret_version, credential_generation, config_version, api_key, fingerprint_version, secret_fingerprint, \
          secret_tail, proxy_profile_id, requests_per_minute, enabled) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(credential.id().to_string())
    .bind(credential.provider_endpoint_id().to_string())
    .bind(credential.label())
    .bind(credential.label_key())
    .bind(credential_kind_text(credential.credential_kind()))
    .bind(to_i64(credential.secret_version())?)
    .bind(to_i64(credential.credential_generation())?)
    .bind(to_i64(credential.config_version())?)
    .bind(api_key.expose_secret())
    .bind(i64::from(credential.fingerprint().version()))
    .bind(credential.fingerprint().digest().as_slice())
    .bind(credential.fingerprint().tail())
    .bind(credential.proxy_profile_id().to_string())
    .bind(
        credential
            .requests_per_minute()
            .map(|value| i64::from(value.get())),
    )
    .bind(credential.enabled())
    .execute(connection)
    .await?;
    Ok(())
}

async fn update_metadata(
    connection: &mut SqliteConnection,
    credential: &ProviderCredential,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE provider_credentials SET label = ?, label_key = ?, proxy_profile_id = ?, \
         requests_per_minute = ?, enabled = ?, credential_generation = ?, config_version = ?, \
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(credential.label())
    .bind(credential.label_key())
    .bind(credential.proxy_profile_id().to_string())
    .bind(
        credential
            .requests_per_minute()
            .map(|value| i64::from(value.get())),
    )
    .bind(credential.enabled())
    .bind(to_i64(credential.credential_generation())?)
    .bind(to_i64(credential.config_version())?)
    .bind(credential.id().to_string())
    .execute(connection)
    .await?;
    require_single_row(result.rows_affected(), credential)
}

async fn rotate_secret(
    connection: &mut SqliteConnection,
    credential: &ProviderCredential,
    api_key: &SecretBytes,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE provider_credentials SET secret_version = ?, credential_generation = ?, \
         config_version = ?, api_key = ?, fingerprint_version = ?, secret_fingerprint = ?, \
         secret_tail = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(to_i64(credential.secret_version())?)
    .bind(to_i64(credential.credential_generation())?)
    .bind(to_i64(credential.config_version())?)
    .bind(api_key.expose_secret())
    .bind(i64::from(credential.fingerprint().version()))
    .bind(credential.fingerprint().digest().as_slice())
    .bind(credential.fingerprint().tail())
    .bind(credential.id().to_string())
    .execute(connection)
    .await?;
    require_single_row(result.rows_affected(), credential)
}

fn require_single_row(
    rows_affected: u64,
    credential: &ProviderCredential,
) -> Result<(), StorageError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(StorageError::ProviderCredentialNotFound(credential.id()))
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::RevisionOverflow)
}

const fn credential_kind_text(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::ApiKey => "api_key",
    }
}
