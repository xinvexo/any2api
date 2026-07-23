use std::{fmt, sync::Arc};

use any2api_domain::ProviderCredential;
use any2api_provider::api::ProviderSecret;

use crate::{
    credential_auth::CredentialAuthMaterial, health::CredentialHealthRuntime,
    scheduler_epoch::SchedulerEpoch,
};

pub struct CredentialGenerationRuntime {
    credential_generation: u64,
    secret_version: u64,
    provider_secret: Arc<ProviderSecret>,
    health: Arc<CredentialHealthRuntime>,
}

impl CredentialGenerationRuntime {
    pub(crate) fn new(
        credential: &ProviderCredential,
        auth_material: CredentialAuthMaterial,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Self {
        assert!(
            auth_material.matches(credential),
            "Credential auth material does not match generation"
        );
        Self {
            credential_generation: credential.credential_generation(),
            secret_version: credential.secret_version(),
            provider_secret: auth_material.into_provider_secret(),
            health: CredentialHealthRuntime::new(scheduler_epoch),
        }
    }

    #[must_use]
    pub const fn credential_generation(&self) -> u64 {
        self.credential_generation
    }

    #[must_use]
    pub const fn secret_version(&self) -> u64 {
        self.secret_version
    }

    pub(crate) fn provider_secret(&self) -> &ProviderSecret {
        self.provider_secret.as_ref()
    }

    pub(crate) fn health(&self) -> &Arc<CredentialHealthRuntime> {
        &self.health
    }

    pub(crate) fn matches(&self, credential: &ProviderCredential) -> bool {
        self.credential_generation == credential.credential_generation()
            && self.secret_version == credential.secret_version()
    }
}

impl fmt::Debug for CredentialGenerationRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialGenerationRuntime")
            .field("credential_generation", &self.credential_generation)
            .field("secret_version", &self.secret_version)
            .field("provider_secret", &"[REDACTED]")
            .finish()
    }
}
