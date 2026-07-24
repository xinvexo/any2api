use std::fmt;

use any2api_domain::{OAuthAccountId, ProviderKind};

use crate::oauth_account_document::OAuthAccountDocument;

pub struct StoredOAuthAccountMaterial {
    account_id: OAuthAccountId,
    provider_kind: ProviderKind,
    token_version: u64,
    account_generation: u64,
    document: OAuthAccountDocument,
}

impl StoredOAuthAccountMaterial {
    pub(crate) const fn new(
        account_id: OAuthAccountId,
        provider_kind: ProviderKind,
        token_version: u64,
        account_generation: u64,
        document: OAuthAccountDocument,
    ) -> Self {
        Self {
            account_id,
            provider_kind,
            token_version,
            account_generation,
            document,
        }
    }

    #[must_use]
    pub const fn account_id(&self) -> OAuthAccountId {
        self.account_id
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub const fn token_version(&self) -> u64 {
        self.token_version
    }

    #[must_use]
    pub const fn account_generation(&self) -> u64 {
        self.account_generation
    }

    #[must_use]
    pub fn into_document(self) -> OAuthAccountDocument {
        self.document
    }

    #[cfg(test)]
    pub(crate) const fn document(&self) -> &OAuthAccountDocument {
        &self.document
    }
}

impl fmt::Debug for StoredOAuthAccountMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredOAuthAccountMaterial")
            .field("account_id", &self.account_id)
            .field("provider_kind", &self.provider_kind)
            .field("token_version", &self.token_version)
            .field("account_generation", &self.account_generation)
            .field("document", &"[REDACTED]")
            .finish()
    }
}

#[derive(Default)]
pub struct StoredOAuthAccountMaterials {
    entries: Vec<StoredOAuthAccountMaterial>,
}

impl StoredOAuthAccountMaterials {
    pub(crate) const fn new(entries: Vec<StoredOAuthAccountMaterial>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn into_entries(self) -> Vec<StoredOAuthAccountMaterial> {
        self.entries
    }

    #[cfg(test)]
    pub(crate) fn get(&self, id: OAuthAccountId) -> Option<&StoredOAuthAccountMaterial> {
        self.entries.iter().find(|entry| entry.account_id == id)
    }
}

impl fmt::Debug for StoredOAuthAccountMaterials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredOAuthAccountMaterials")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}
