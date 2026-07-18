use any2api_domain::{CredentialKind, ProviderCredential, ProviderEndpointId};
use sqlx::SqliteConnection;

use crate::{
    error::StorageError, provider_credential_mutation::ProviderCredentialDatabaseChange,
    vault::SecretEnvelope,
};

pub(crate) async fn execute_provider_credential_change(
    connection: &mut SqliteConnection,
    change: &ProviderCredentialDatabaseChange,
) -> Result<(), StorageError> {
    match change {
        ProviderCredentialDatabaseChange::Create {
            credential,
            envelope,
        } => insert(connection, credential, envelope).await?,
        ProviderCredentialDatabaseChange::Update(credential) => {
            update_metadata(connection, credential).await?
        }
        ProviderCredentialDatabaseChange::RotateSecret {
            credential,
            envelope,
        } => rotate_secret(connection, credential, envelope).await?,
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
    envelope: &SecretEnvelope,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO provider_credentials \
         (id, provider_endpoint_id, label, label_key, credential_kind, secret_schema_version, \
          secret_version, credential_generation, config_version, envelope_version, key_id, \
          algorithm, nonce, ciphertext, aad_version, fingerprint_version, secret_fingerprint, \
          secret_tail, proxy_profile_id, max_concurrency, enabled) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(credential.id().to_string())
    .bind(credential.provider_endpoint_id().to_string())
    .bind(credential.label())
    .bind(credential.label_key())
    .bind(credential_kind_text(credential.credential_kind()))
    .bind(i64::from(credential.secret_schema_version()))
    .bind(to_i64(credential.secret_version())?)
    .bind(to_i64(credential.credential_generation())?)
    .bind(to_i64(credential.config_version())?)
    .bind(i64::from(envelope.version()))
    .bind(envelope.key_id())
    .bind(envelope.algorithm().as_str())
    .bind(envelope.nonce().as_slice())
    .bind(envelope.ciphertext())
    .bind(i64::from(envelope.aad_version()))
    .bind(i64::from(credential.fingerprint().version()))
    .bind(credential.fingerprint().digest().as_slice())
    .bind(credential.fingerprint().tail())
    .bind(credential.proxy_profile_id().to_string())
    .bind(i64::from(credential.max_concurrency().get()))
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
         max_concurrency = ?, enabled = ?, credential_generation = ?, config_version = ?, \
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(credential.label())
    .bind(credential.label_key())
    .bind(credential.proxy_profile_id().to_string())
    .bind(i64::from(credential.max_concurrency().get()))
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
    envelope: &SecretEnvelope,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE provider_credentials SET secret_version = ?, credential_generation = ?, \
         config_version = ?, envelope_version = ?, key_id = ?, algorithm = ?, nonce = ?, \
         ciphertext = ?, aad_version = ?, fingerprint_version = ?, secret_fingerprint = ?, \
         secret_tail = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(to_i64(credential.secret_version())?)
    .bind(to_i64(credential.credential_generation())?)
    .bind(to_i64(credential.config_version())?)
    .bind(i64::from(envelope.version()))
    .bind(envelope.key_id())
    .bind(envelope.algorithm().as_str())
    .bind(envelope.nonce().as_slice())
    .bind(envelope.ciphertext())
    .bind(i64::from(envelope.aad_version()))
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
