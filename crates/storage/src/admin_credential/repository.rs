use async_trait::async_trait;

use crate::{
    error::StorageError,
    sqlite::SqliteStore,
    sqlite_commit::{self, SqliteCommitError},
};

#[derive(Clone, Eq, PartialEq)]
pub struct StoredAdminCredential {
    password_hash: String,
}

impl StoredAdminCredential {
    pub(crate) fn new(password_hash: String) -> Self {
        Self { password_hash }
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }
}

impl std::fmt::Debug for StoredAdminCredential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StoredAdminCredential")
            .field("password_hash", &"[redacted]")
            .finish()
    }
}

#[async_trait]
pub trait AdminCredentialRepository: Send + Sync {
    async fn load_admin_credential(&self) -> Result<Option<StoredAdminCredential>, StorageError>;

    async fn initialize_admin_credential(&self, password_hash: &str) -> Result<bool, StorageError>;

    async fn replace_admin_credential(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, StorageError>;
}

#[async_trait]
impl AdminCredentialRepository for SqliteStore {
    async fn load_admin_credential(&self) -> Result<Option<StoredAdminCredential>, StorageError> {
        let row = sqlx::query_scalar::<_, String>(
            "SELECT password_hash FROM admin_credentials WHERE singleton = 1",
        )
        .fetch_optional(self.pool())
        .await?;
        row.map(|password_hash| {
            if password_hash.is_empty() {
                Err(StorageError::CorruptConfiguration)
            } else {
                Ok(StoredAdminCredential::new(password_hash))
            }
        })
        .transpose()
    }

    async fn initialize_admin_credential(&self, password_hash: &str) -> Result<bool, StorageError> {
        let mut transaction = self.begin_write().await?;
        let result =
            sqlx::query("INSERT INTO admin_credentials (singleton, password_hash) VALUES (1, ?)")
                .bind(password_hash)
                .execute(&mut *transaction)
                .await;
        match result {
            Ok(result) => {
                let changed = result.rows_affected() == 1;
                commit_admin_credential_transaction(transaction).await?;
                Ok(changed)
            }
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                transaction.rollback().await?;
                Ok(false)
            }
            Err(error) => {
                transaction.rollback().await?;
                Err(error.into())
            }
        }
    }

    async fn replace_admin_credential(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, StorageError> {
        let mut transaction = self.begin_write().await?;
        let result = sqlx::query(
            "UPDATE admin_credentials
             SET password_hash = ?, updated_at = CURRENT_TIMESTAMP
             WHERE singleton = 1 AND password_hash = ?",
        )
        .bind(new_password_hash)
        .bind(expected_password_hash)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(false);
        }
        commit_admin_credential_transaction(transaction).await?;
        Ok(true)
    }
}

async fn commit_admin_credential_transaction(
    transaction: sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<(), StorageError> {
    sqlite_commit::commit(transaction)
        .await
        .map_err(map_admin_credential_commit_error)
}

pub(super) fn map_admin_credential_commit_error(error: SqliteCommitError) -> StorageError {
    match error {
        SqliteCommitError::Determinate(source) => StorageError::Database(source),
        SqliteCommitError::Indeterminate(source) => {
            StorageError::IndeterminateAdminCredentialCommit { source }
        }
    }
}
