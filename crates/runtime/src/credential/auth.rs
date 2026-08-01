use std::{collections::HashMap, fmt, sync::Arc};

use any2api_domain::{CredentialId, CredentialKind, ProviderCredential};
use any2api_provider::api::ProviderSecret;
use any2api_storage::api::{StoredProviderCredentialSecret, StoredProviderCredentialSecrets};
use secrecy::ExposeSecret;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CredentialAuthMaterialError {
    #[error("provider API Key material is not valid UTF-8 for credential {0}")]
    InvalidUtf8(CredentialId),
    #[error("duplicate provider API Key material for credential {0}")]
    Duplicate(CredentialId),
    #[error("provider API Key material is missing for credential {0}")]
    Missing(CredentialId),
    #[error("provider API Key kind does not match credential {0}")]
    KindMismatch(CredentialId),
    #[error("provider API Key version does not match credential {0}")]
    VersionMismatch(CredentialId),
    #[error("provider API Key generation does not match credential {0}")]
    GenerationMismatch(CredentialId),
    #[error("provider API Key material references unknown credential {0}")]
    Unknown(CredentialId),
}

pub(crate) struct CredentialAuthMaterial {
    credential_id: CredentialId,
    credential_kind: CredentialKind,
    secret_version: u64,
    credential_generation: u64,
    provider_secret: Arc<ProviderSecret>,
}

impl CredentialAuthMaterial {
    fn from_stored(
        stored: StoredProviderCredentialSecret,
    ) -> Result<Self, CredentialAuthMaterialError> {
        let credential_id = stored.credential_id();
        let credential_kind = stored.credential_kind();
        let secret_version = stored.secret_version();
        let credential_generation = stored.credential_generation();
        let secret = stored.into_secret();
        let value = String::from_utf8(secret.expose_secret().to_vec())
            .map_err(|_| CredentialAuthMaterialError::InvalidUtf8(credential_id))?;
        Ok(Self {
            credential_id,
            credential_kind,
            secret_version,
            credential_generation,
            provider_secret: Arc::new(ProviderSecret::new(value)),
        })
    }

    #[cfg(test)]
    pub(crate) fn for_test(credential: &ProviderCredential, value: String) -> Self {
        Self {
            credential_id: credential.id(),
            credential_kind: credential.credential_kind(),
            secret_version: credential.secret_version(),
            credential_generation: credential.credential_generation(),
            provider_secret: Arc::new(ProviderSecret::new(value)),
        }
    }

    #[cfg(test)]
    pub(crate) fn matches(&self, credential: &ProviderCredential) -> bool {
        self.credential_id == credential.id()
            && self.credential_kind == credential.credential_kind()
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
    pub(crate) fn from_stored(
        stored: StoredProviderCredentialSecrets,
    ) -> Result<Self, CredentialAuthMaterialError> {
        let mut by_id = HashMap::new();
        for entry in stored.into_entries() {
            let material = CredentialAuthMaterial::from_stored(entry)?;
            let id = material.credential_id;
            if by_id.insert(id, material).is_some() {
                return Err(CredentialAuthMaterialError::Duplicate(id));
            }
        }
        Ok(Self { by_id })
    }

    pub(crate) fn take_for(
        &mut self,
        credential: &ProviderCredential,
    ) -> Result<CredentialAuthMaterial, CredentialAuthMaterialError> {
        let material = self
            .by_id
            .remove(&credential.id())
            .ok_or(CredentialAuthMaterialError::Missing(credential.id()))?;
        if material.credential_kind != credential.credential_kind() {
            return Err(CredentialAuthMaterialError::KindMismatch(credential.id()));
        }
        if material.secret_version != credential.secret_version() {
            return Err(CredentialAuthMaterialError::VersionMismatch(
                credential.id(),
            ));
        }
        if material.credential_generation != credential.credential_generation() {
            return Err(CredentialAuthMaterialError::GenerationMismatch(
                credential.id(),
            ));
        }
        Ok(material)
    }

    pub(crate) fn ensure_consumed(self) -> Result<(), CredentialAuthMaterialError> {
        self.by_id
            .keys()
            .next()
            .copied()
            .map_or(Ok(()), |id| Err(CredentialAuthMaterialError::Unknown(id)))
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
