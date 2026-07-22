use any2api_domain::GatewayApiKeyId;
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{error::StorageError, sqlite::SqliteStore};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyLastUsedUpdate {
    pub id: GatewayApiKeyId,
    pub last_used_at: String,
}

#[async_trait]
pub trait GatewayApiKeyUsageRepository: Send + Sync {
    async fn touch_gateway_api_key_last_used(
        &self,
        updates: &[GatewayApiKeyLastUsedUpdate],
    ) -> Result<(), StorageError>;
}

#[async_trait]
impl GatewayApiKeyUsageRepository for SqliteStore {
    async fn touch_gateway_api_key_last_used(
        &self,
        updates: &[GatewayApiKeyLastUsedUpdate],
    ) -> Result<(), StorageError> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        for update in updates {
            touch_one(&mut transaction, update).await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

async fn touch_one(
    connection: &mut SqliteConnection,
    update: &GatewayApiKeyLastUsedUpdate,
) -> Result<(), StorageError> {
    sqlx::query(
        "UPDATE gateway_api_keys \
         SET last_used_at = ? \
         WHERE id = ? \
           AND (last_used_at IS NULL OR last_used_at < ?)",
    )
    .bind(&update.last_used_at)
    .bind(update.id.to_string())
    .bind(&update.last_used_at)
    .execute(&mut *connection)
    .await?;
    Ok(())
}
