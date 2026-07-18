use any2api_domain::GatewayApiKey;
use sqlx::SqliteConnection;

use crate::{error::StorageError, gateway_api_key_mutation::GatewayApiKeyDatabaseChange};

pub(crate) async fn execute_change(
    connection: &mut SqliteConnection,
    change: &GatewayApiKeyDatabaseChange,
) -> Result<(), StorageError> {
    match change {
        GatewayApiKeyDatabaseChange::Create(key) => insert(connection, key).await?,
        GatewayApiKeyDatabaseChange::Update(key) => update(connection, key).await?,
        GatewayApiKeyDatabaseChange::Rotate(key) => rotate(connection, key).await?,
        GatewayApiKeyDatabaseChange::Revoke(key) => revoke(connection, key).await?,
    }
    Ok(())
}

async fn insert(
    connection: &mut SqliteConnection,
    key: &GatewayApiKey,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO gateway_api_keys \
         (id, name, name_key, token_prefix, token_hash, hash_version, hash_key_id, \
          token_version, config_version, enabled, revoked_at, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(key.id().to_string())
    .bind(key.name())
    .bind(key.name_key())
    .bind(key.token_prefix())
    .bind(key.token_hash().as_slice())
    .bind(i64::from(key.hash_version()))
    .bind(key.hash_key_id())
    .bind(to_i64(key.token_version())?)
    .bind(to_i64(key.config_version())?)
    .bind(key.enabled())
    .bind(key.revoked_at())
    .bind(key.created_at())
    .execute(connection)
    .await?;
    Ok(())
}

async fn update(
    connection: &mut SqliteConnection,
    key: &GatewayApiKey,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE gateway_api_keys SET name = ?, name_key = ?, enabled = ?, \
         config_version = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(key.name())
    .bind(key.name_key())
    .bind(key.enabled())
    .bind(to_i64(key.config_version())?)
    .bind(key.id().to_string())
    .execute(connection)
    .await?;
    require_single_row(result.rows_affected(), key)
}

async fn rotate(
    connection: &mut SqliteConnection,
    key: &GatewayApiKey,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE gateway_api_keys SET token_prefix = ?, token_hash = ?, hash_version = ?, \
         hash_key_id = ?, token_version = ?, config_version = ?, updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?",
    )
    .bind(key.token_prefix())
    .bind(key.token_hash().as_slice())
    .bind(i64::from(key.hash_version()))
    .bind(key.hash_key_id())
    .bind(to_i64(key.token_version())?)
    .bind(to_i64(key.config_version())?)
    .bind(key.id().to_string())
    .execute(connection)
    .await?;
    require_single_row(result.rows_affected(), key)
}

async fn revoke(
    connection: &mut SqliteConnection,
    key: &GatewayApiKey,
) -> Result<(), StorageError> {
    let result = sqlx::query(
        "UPDATE gateway_api_keys SET enabled = 0, revoked_at = ?, config_version = ?, \
         updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(key.revoked_at())
    .bind(to_i64(key.config_version())?)
    .bind(key.id().to_string())
    .execute(connection)
    .await?;
    require_single_row(result.rows_affected(), key)
}

fn require_single_row(rows_affected: u64, key: &GatewayApiKey) -> Result<(), StorageError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(StorageError::GatewayApiKeyNotFound(key.id()))
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::RevisionOverflow)
}
