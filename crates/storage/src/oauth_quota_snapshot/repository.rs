use any2api_domain::OAuthAccountId;
use async_trait::async_trait;
use sqlx::FromRow;

use crate::{error::StorageError, sqlite::SqliteStore};

pub const OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION: u32 = 5;
pub const MAX_OAUTH_QUOTA_SNAPSHOT_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredOAuthQuotaSnapshot {
    pub oauth_account_id: OAuthAccountId,
    pub schema_version: u32,
    pub fetched_at: i64,
    pub payload: Vec<u8>,
}

#[async_trait]
pub trait OAuthQuotaSnapshotRepository: Send + Sync {
    async fn load_oauth_quota_snapshot(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<StoredOAuthQuotaSnapshot>, StorageError>;

    async fn upsert_oauth_quota_snapshot(
        &self,
        snapshot: &StoredOAuthQuotaSnapshot,
    ) -> Result<(), StorageError>;

    async fn delete_oauth_quota_snapshot(&self, id: OAuthAccountId) -> Result<bool, StorageError>;
}

#[async_trait]
impl OAuthQuotaSnapshotRepository for SqliteStore {
    async fn load_oauth_quota_snapshot(
        &self,
        id: OAuthAccountId,
    ) -> Result<Option<StoredOAuthQuotaSnapshot>, StorageError> {
        let row = sqlx::query_as::<_, SnapshotRow>(
            "SELECT oauth_account_id, schema_version, fetched_at, payload \
             FROM oauth_quota_snapshots WHERE oauth_account_id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(self.pool())
        .await?;
        row.map(parse_row).transpose()
    }

    async fn upsert_oauth_quota_snapshot(
        &self,
        snapshot: &StoredOAuthQuotaSnapshot,
    ) -> Result<(), StorageError> {
        validate(snapshot)?;
        sqlx::query(
            "INSERT INTO oauth_quota_snapshots \
             (oauth_account_id, schema_version, fetched_at, payload) VALUES (?, ?, ?, ?) \
             ON CONFLICT(oauth_account_id) DO UPDATE SET \
                 schema_version = excluded.schema_version, \
                 fetched_at = excluded.fetched_at, \
                 payload = excluded.payload, \
                 updated_at = CURRENT_TIMESTAMP",
        )
        .bind(snapshot.oauth_account_id.to_string())
        .bind(i64::from(snapshot.schema_version))
        .bind(snapshot.fetched_at)
        .bind(&snapshot.payload)
        .execute(self.write_pool())
        .await?;
        Ok(())
    }

    async fn delete_oauth_quota_snapshot(&self, id: OAuthAccountId) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM oauth_quota_snapshots WHERE oauth_account_id = ?")
            .bind(id.to_string())
            .execute(self.write_pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(FromRow)]
struct SnapshotRow {
    oauth_account_id: String,
    schema_version: i64,
    fetched_at: i64,
    payload: Vec<u8>,
}

fn parse_row(row: SnapshotRow) -> Result<StoredOAuthQuotaSnapshot, StorageError> {
    let snapshot = StoredOAuthQuotaSnapshot {
        oauth_account_id: row
            .oauth_account_id
            .parse()
            .map_err(|_| StorageError::CorruptOAuthQuotaSnapshot)?,
        schema_version: u32::try_from(row.schema_version)
            .map_err(|_| StorageError::CorruptOAuthQuotaSnapshot)?,
        fetched_at: row.fetched_at,
        payload: row.payload,
    };
    validate(&snapshot)?;
    Ok(snapshot)
}

fn validate(snapshot: &StoredOAuthQuotaSnapshot) -> Result<(), StorageError> {
    if snapshot.schema_version != OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION
        || snapshot.fetched_at < 0
        || !(2..=MAX_OAUTH_QUOTA_SNAPSHOT_BYTES).contains(&snapshot.payload.len())
    {
        return Err(StorageError::CorruptOAuthQuotaSnapshot);
    }
    Ok(())
}
