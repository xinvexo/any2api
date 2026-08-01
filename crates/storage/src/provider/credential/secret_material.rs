use std::fmt;

use any2api_domain::{CredentialId, CredentialKind};

use crate::secret::SecretBytes;

pub struct StoredProviderCredentialSecret {
    credential_id: CredentialId,
    credential_kind: CredentialKind,
    secret_version: u64,
    credential_generation: u64,
    secret: SecretBytes,
}

impl StoredProviderCredentialSecret {
    pub(crate) const fn new(
        credential_id: CredentialId,
        credential_kind: CredentialKind,
        secret_version: u64,
        credential_generation: u64,
        secret: SecretBytes,
    ) -> Self {
        Self {
            credential_id,
            credential_kind,
            secret_version,
            credential_generation,
            secret,
        }
    }

    #[must_use]
    pub const fn credential_id(&self) -> CredentialId {
        self.credential_id
    }

    #[must_use]
    pub const fn credential_kind(&self) -> CredentialKind {
        self.credential_kind
    }

    #[must_use]
    pub const fn secret_version(&self) -> u64 {
        self.secret_version
    }

    #[must_use]
    pub const fn credential_generation(&self) -> u64 {
        self.credential_generation
    }

    #[must_use]
    pub fn into_secret(self) -> SecretBytes {
        self.secret
    }

    #[cfg(test)]
    pub(crate) fn expose_for_test(&self) -> &[u8] {
        use secrecy::ExposeSecret;

        self.secret.expose_secret()
    }
}

impl fmt::Debug for StoredProviderCredentialSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredProviderCredentialSecret")
            .field("credential_id", &self.credential_id)
            .field("credential_kind", &self.credential_kind)
            .field("secret_version", &self.secret_version)
            .field("credential_generation", &self.credential_generation)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Default)]
pub struct StoredProviderCredentialSecrets {
    entries: Vec<StoredProviderCredentialSecret>,
}

impl StoredProviderCredentialSecrets {
    pub(crate) const fn new(entries: Vec<StoredProviderCredentialSecret>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn into_entries(self) -> Vec<StoredProviderCredentialSecret> {
        self.entries
    }

    #[cfg(test)]
    pub(crate) fn get(&self, id: CredentialId) -> Option<&StoredProviderCredentialSecret> {
        self.entries.iter().find(|entry| entry.credential_id == id)
    }
}

impl fmt::Debug for StoredProviderCredentialSecrets {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredProviderCredentialSecrets")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}
