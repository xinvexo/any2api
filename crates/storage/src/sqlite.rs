use std::path::Path;

use any2api_domain::ConfigRevision;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

use crate::{error::StorageError, migration};

#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn connect(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|source| {
                StorageError::CreateDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

        migration::run(&pool).await?;
        Ok(Self { pool })
    }

    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn load_config_revision(&self) -> Result<ConfigRevision, StorageError> {
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM config_state WHERE singleton_id = 1")
                .fetch_one(&self.pool)
                .await?;
        let revision = u64::try_from(revision)
            .ok()
            .and_then(|value| ConfigRevision::new(value).ok())
            .ok_or(StorageError::InvalidRevision(revision))?;

        Ok(revision)
    }
}
