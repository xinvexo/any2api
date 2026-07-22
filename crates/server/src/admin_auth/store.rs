use std::error::Error;

use async_trait::async_trait;

pub type AdminCredentialStoreError = Box<dyn Error + Send + Sync + 'static>;

#[derive(Clone, Eq, PartialEq)]
pub struct StoredAdminPasswordHash(String);

impl StoredAdminPasswordHash {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for StoredAdminPasswordHash {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("StoredAdminPasswordHash([redacted])")
    }
}

#[async_trait]
pub trait AdminCredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<StoredAdminPasswordHash>, AdminCredentialStoreError>;

    async fn initialize(&self, password_hash: &str) -> Result<bool, AdminCredentialStoreError>;

    async fn replace(
        &self,
        expected_password_hash: &str,
        new_password_hash: &str,
    ) -> Result<bool, AdminCredentialStoreError>;
}
