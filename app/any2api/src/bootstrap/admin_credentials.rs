use std::sync::Arc;

use any2api_server::api::{
    AdminCredentialStore, AdminCredentialStoreError, StoredAdminPasswordHash,
};
use any2api_storage::api::{AdminCredentialRepository, SqliteStore, StorageError};
use async_trait::async_trait;

pub(crate) struct SqliteAdminCredentialStore {
    storage: Arc<SqliteStore>,
}

impl SqliteAdminCredentialStore {
    pub(crate) fn new(storage: Arc<SqliteStore>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl AdminCredentialStore for SqliteAdminCredentialStore {
    async fn load(&self) -> Result<Option<StoredAdminPasswordHash>, AdminCredentialStoreError> {
        self.storage
            .load_admin_credential()
            .await
            .map(|credential| {
                credential.map(|credential| {
                    StoredAdminPasswordHash::new(credential.password_hash().to_owned())
                })
            })
            .map_err(AdminCredentialStoreError::operation)
    }

    async fn initialize(&self, password_hash: &str) -> Result<bool, AdminCredentialStoreError> {
        self.storage
            .initialize_admin_credential(password_hash)
            .await
            .map_err(map_write_error)
    }

    async fn replace(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, AdminCredentialStoreError> {
        self.storage
            .replace_admin_credential(expected_password_hash, new_password_hash)
            .await
            .map_err(map_write_error)
    }
}

fn map_write_error(error: StorageError) -> AdminCredentialStoreError {
    match error {
        StorageError::IndeterminateAdminCredentialCommit { source } => {
            AdminCredentialStoreError::indeterminate_commit(source)
        }
        error => AdminCredentialStoreError::operation(error),
    }
}
