use std::str::FromStr;

use any2api_domain::{
    GatewayApiKey, GatewayApiKeyConfiguration, GatewayApiKeyDraft, GatewayApiKeyId,
};
use sqlx::{FromRow, SqliteConnection};

use crate::{error::StorageError, gateway_api_key_verifier::GatewayApiKeyVerifier};

#[derive(FromRow)]
struct GatewayApiKeyRow {
    id: String,
    name: String,
    token_prefix: String,
    token_hash: Vec<u8>,
    hash_version: i64,
    hash_key_id: String,
    token_version: i64,
    config_version: i64,
    enabled: i64,
    revoked_at: Option<String>,
    created_at: String,
    last_used_at: Option<String>,
}

pub(crate) async fn load_gateway_api_keys_from(
    connection: &mut SqliteConnection,
    verifier: &GatewayApiKeyVerifier,
) -> Result<GatewayApiKeyConfiguration, StorageError> {
    let rows = sqlx::query_as::<_, GatewayApiKeyRow>(
        "SELECT id, name, token_prefix, token_hash, hash_version, hash_key_id, token_version, \
         config_version, enabled, revoked_at, created_at, last_used_at \
         FROM gateway_api_keys ORDER BY name ASC",
    )
    .fetch_all(connection)
    .await?;
    let keys = rows
        .into_iter()
        .map(|row| parse_row(row, verifier))
        .collect::<Result<Vec<_>, _>>()?;
    GatewayApiKeyConfiguration::new(keys).map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_row(
    row: GatewayApiKeyRow,
    verifier: &GatewayApiKeyVerifier,
) -> Result<GatewayApiKey, StorageError> {
    if row.hash_key_id != verifier.key_id() {
        return Err(StorageError::GatewayApiKeyHashKeyMismatch);
    }
    let id = GatewayApiKeyId::from_str(&row.id).map_err(|_| StorageError::CorruptConfiguration)?;
    let token_hash: [u8; 32] = row
        .token_hash
        .try_into()
        .map_err(|_| StorageError::CorruptConfiguration)?;
    let draft = GatewayApiKeyDraft::new(row.name, parse_bool(row.enabled)?)
        .map_err(|_| StorageError::CorruptConfiguration)?;
    GatewayApiKey::restore(
        id,
        draft,
        row.token_prefix,
        token_hash,
        u32::try_from(row.hash_version).map_err(|_| StorageError::CorruptConfiguration)?,
        row.hash_key_id,
        parse_version(row.token_version)?,
        parse_version(row.config_version)?,
        row.revoked_at,
        row.created_at,
        row.last_used_at,
    )
    .map_err(|_| StorageError::CorruptConfiguration)
}

fn parse_bool(value: i64) -> Result<bool, StorageError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(StorageError::CorruptConfiguration),
    }
}

fn parse_version(value: i64) -> Result<u64, StorageError> {
    let value = u64::try_from(value).map_err(|_| StorageError::CorruptConfiguration)?;
    (value > 0 && value <= u64::from(u32::MAX))
        .then_some(value)
        .ok_or(StorageError::CorruptConfiguration)
}
