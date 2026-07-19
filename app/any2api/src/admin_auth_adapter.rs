use std::sync::Arc;

use any2api_server::api::{
    AdminCredentialStore, AdminCredentialStoreError, StoredAdminPasswordHash,
};
use any2api_storage::api::{AdminCredentialRepository, SqliteStore};
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
            .map_err(|error| Box::new(error) as AdminCredentialStoreError)
    }

    async fn initialize(&self, password_hash: &str) -> Result<bool, AdminCredentialStoreError> {
        self.storage
            .initialize_admin_credential(password_hash)
            .await
            .map_err(|error| Box::new(error) as AdminCredentialStoreError)
    }
}
