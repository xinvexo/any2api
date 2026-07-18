use std::path::Path;

use sqlx::{FromRow, SqliteConnection, SqlitePool};

use super::{
    cipher::SecretVault, envelope::SecretEnvelope, error::SecretVaultError, master_key::MasterKey,
};

pub(crate) async fn initialize_vault(
    pool: &SqlitePool,
    master_key_path: &Path,
) -> Result<SecretVault, SecretVaultError> {
    let mut transaction = pool.begin_with("BEGIN IMMEDIATE").await?;
    let stored = load_metadata(&mut transaction).await?;
    let master_key = MasterKey::load_or_create(master_key_path, stored.is_none())?;
    let vault = SecretVault::new(master_key);

    if let Some(envelope) = stored {
        vault.verify(&envelope)?;
    } else {
        let envelope = vault.seal_verifier()?;
        insert_metadata(&mut transaction, &envelope).await?;
    }
    transaction.commit().await?;
    Ok(vault)
}

#[derive(Debug, FromRow)]
struct VaultMetadataRow {
    envelope_version: i64,
    key_id: String,
    algorithm: String,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    aad_version: i64,
}

async fn load_metadata(
    connection: &mut SqliteConnection,
) -> Result<Option<SecretEnvelope>, SecretVaultError> {
    let row = sqlx::query_as::<_, VaultMetadataRow>(
        "SELECT envelope_version, key_id, algorithm, nonce, ciphertext, aad_version \
         FROM secret_vault_metadata WHERE singleton_id = 1",
    )
    .fetch_optional(connection)
    .await?;
    row.map(parse_row).transpose()
}

fn parse_row(row: VaultMetadataRow) -> Result<SecretEnvelope, SecretVaultError> {
    let version =
        u16::try_from(row.envelope_version).map_err(|_| SecretVaultError::InvalidEnvelope)?;
    let aad_version =
        u16::try_from(row.aad_version).map_err(|_| SecretVaultError::InvalidEnvelope)?;
    SecretEnvelope::restore(
        version,
        row.key_id,
        &row.algorithm,
        &row.nonce,
        row.ciphertext,
        aad_version,
    )
}

async fn insert_metadata(
    connection: &mut SqliteConnection,
    envelope: &SecretEnvelope,
) -> Result<(), SecretVaultError> {
    sqlx::query(
        "INSERT INTO secret_vault_metadata \
         (singleton_id, envelope_version, key_id, algorithm, nonce, ciphertext, aad_version) \
         VALUES (1, ?, ?, ?, ?, ?, ?)",
    )
    .bind(i64::from(envelope.version()))
    .bind(envelope.key_id())
    .bind(envelope.algorithm().as_str())
    .bind(envelope.nonce().as_slice())
    .bind(envelope.ciphertext())
    .bind(i64::from(envelope.aad_version()))
    .execute(connection)
    .await?;
    Ok(())
}
