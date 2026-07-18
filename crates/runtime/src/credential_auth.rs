use std::{collections::HashMap, fmt, sync::Arc};

use any2api_domain::{CredentialId, CredentialKind, ProviderCredential};
use any2api_provider::api::ProviderSecret;
use any2api_storage::api::{StoredProviderCredentialSecret, StoredProviderCredentialSecrets};
use secrecy::ExposeSecret;

pub(crate) struct CredentialAuthMaterial {
    credential_id: CredentialId,
    credential_kind: CredentialKind,
    secret_schema_version: u32,
    secret_version: u64,
    credential_generation: u64,
    provider_secret: Arc<ProviderSecret>,
}

impl CredentialAuthMaterial {
    fn from_stored(stored: StoredProviderCredentialSecret) -> Self {
        let credential_id = stored.credential_id();
        let credential_kind = stored.credential_kind();
        let secret_schema_version = stored.secret_schema_version();
        let secret_version = stored.secret_version();
        let credential_generation = stored.credential_generation();
        let secret = stored.into_secret();
        let value = String::from_utf8(secret.expose_secret().to_vec())
            .expect("storage validated Provider API Key as visible ASCII");
        Self {
            credential_id,
            credential_kind,
            secret_schema_version,
            secret_version,
            credential_generation,
            provider_secret: Arc::new(ProviderSecret::new(secret_schema_version, value)),
        }
    }

    #[cfg(test)]
    fn for_test(credential: &ProviderCredential, value: String) -> Self {
        Self {
            credential_id: credential.id(),
            credential_kind: credential.credential_kind(),
            secret_schema_version: credential.secret_schema_version(),
            secret_version: credential.secret_version(),
            credential_generation: credential.credential_generation(),
            provider_secret: Arc::new(ProviderSecret::new(
                credential.secret_schema_version(),
                value,
            )),
        }
    }

    pub(crate) fn matches(&self, credential: &ProviderCredential) -> bool {
        self.credential_id == credential.id()
            && self.credential_kind == credential.credential_kind()
            && self.secret_schema_version == credential.secret_schema_version()
            && self.secret_version == credential.secret_version()
            && self.credential_generation == credential.credential_generation()
    }

    pub(crate) fn into_provider_secret(self) -> Arc<ProviderSecret> {
        self.provider_secret
    }
}

impl fmt::Debug for CredentialAuthMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialAuthMaterial")
            .field("credential_id", &self.credential_id)
            .field("credential_kind", &self.credential_kind)
            .field("secret_schema_version", &self.secret_schema_version)
            .field("secret_version", &self.secret_version)
            .field("credential_generation", &self.credential_generation)
            .field("provider_secret", &"[REDACTED]")
            .finish()
    }
}

pub(crate) struct CredentialAuthMaterials {
    by_id: HashMap<CredentialId, CredentialAuthMaterial>,
}

impl CredentialAuthMaterials {
    pub(crate) fn from_stored(stored: StoredProviderCredentialSecrets) -> Self {
        let mut by_id = HashMap::new();
        for entry in stored.into_entries() {
            let material = CredentialAuthMaterial::from_stored(entry);
            assert!(
                by_id.insert(material.credential_id, material).is_none(),
                "storage returned duplicate Credential auth material"
            );
        }
        Self { by_id }
    }

    pub(crate) fn take_for(&mut self, credential: &ProviderCredential) -> CredentialAuthMaterial {
        let material = self
            .by_id
            .remove(&credential.id())
            .expect("storage omitted Credential auth material");
        assert!(
            material.matches(credential),
            "Credential auth material version does not match configuration"
        );
        material
    }

    pub(crate) fn assert_consumed(self) {
        assert!(
            self.by_id.is_empty(),
            "storage returned auth material for an unknown Credential"
        );
    }

    #[cfg(test)]
    pub(crate) fn for_configuration<F>(
        configuration: &any2api_domain::ProviderCredentialConfiguration,
        mut secret: F,
    ) -> Self
    where
        F: FnMut(&ProviderCredential) -> String,
    {
        let by_id = configuration
            .credentials()
            .iter()
            .map(|credential| {
                (
                    credential.id(),
                    CredentialAuthMaterial::for_test(credential, secret(credential)),
                )
            })
            .collect();
        Self { by_id }
    }
}

impl fmt::Debug for CredentialAuthMaterials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialAuthMaterials")
            .field("entry_count", &self.by_id.len())
            .finish()
    }
}
