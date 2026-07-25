use std::fmt;

use any2api_domain::ProxyProfileId;

use crate::vault::SecretBytes;

pub struct StoredProxyPassword {
    proxy_profile_id: ProxyProfileId,
    authentication_version: u64,
    secret: SecretBytes,
}

impl StoredProxyPassword {
    pub(crate) const fn new(
        proxy_profile_id: ProxyProfileId,
        authentication_version: u64,
        secret: SecretBytes,
    ) -> Self {
        Self {
            proxy_profile_id,
            authentication_version,
            secret,
        }
    }

    #[must_use]
    pub const fn proxy_profile_id(&self) -> ProxyProfileId {
        self.proxy_profile_id
    }

    #[must_use]
    pub const fn authentication_version(&self) -> u64 {
        self.authentication_version
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

impl fmt::Debug for StoredProxyPassword {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredProxyPassword")
            .field("proxy_profile_id", &self.proxy_profile_id)
            .field("authentication_version", &self.authentication_version)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Default)]
pub struct StoredProxyPasswords {
    entries: Vec<StoredProxyPassword>,
}

impl StoredProxyPasswords {
    pub(crate) const fn new(entries: Vec<StoredProxyPassword>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn into_entries(self) -> Vec<StoredProxyPassword> {
        self.entries
    }

    #[cfg(test)]
    pub(crate) fn get(&self, id: ProxyProfileId) -> Option<&StoredProxyPassword> {
        self.entries
            .iter()
            .find(|entry| entry.proxy_profile_id == id)
    }
}

impl fmt::Debug for StoredProxyPasswords {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredProxyPasswords")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}
