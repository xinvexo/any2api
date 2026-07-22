use std::{path::Path, sync::Arc};

use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

use crate::{
    error::StorageError,
    migration,
    vault::{SecretVault, SecretVaultError, initialize_vault},
};

#[derive(Clone, Debug)]
pub struct SqliteStore {
    pool: SqlitePool,
    secret_vault: Arc<SecretVault>,
}

impl SqliteStore {
    pub async fn connect(path: &Path) -> Result<Self, StorageError> {
        let master_key_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("master-key.json");
        Self::connect_with_master_key(path, &master_key_path).await
    }

    pub async fn connect_with_master_key(
        path: &Path,
        master_key_path: &Path,
    ) -> Result<Self, StorageError> {
        if path == master_key_path {
            return Err(SecretVaultError::MasterKeyPathConflictsWithDatabase.into());
        }
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
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
        let secret_vault = Arc::new(initialize_vault(&pool, master_key_path).await?);
        Ok(Self { pool, secret_vault })
    }

    #[must_use]
    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    #[must_use]
    pub fn secret_vault(&self) -> &SecretVault {
        &self.secret_vault
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}
