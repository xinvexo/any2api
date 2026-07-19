use async_trait::async_trait;

use crate::{error::StorageError, sqlite::SqliteStore};

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
        let result =
            sqlx::query("INSERT INTO admin_credentials (singleton, password_hash) VALUES (1, ?)")
                .bind(password_hash)
                .execute(self.pool())
                .await;
        match result {
            Ok(result) => Ok(result.rows_affected() == 1),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}
