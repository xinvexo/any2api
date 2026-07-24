use std::{fmt, sync::Arc};

use any2api_provider::api::{
    CredentialHeaders, OAuthTokenMaterial, ProviderDriver, ProviderError, ProviderSecret,
};
use http::HeaderMap;

use crate::{health::CredentialHealthRuntime, scheduler_epoch::SchedulerEpoch};

pub(crate) struct CredentialGenerationDefinition {
    routing_generation: u64,
    authentication_version: u64,
    authentication: CredentialAuthentication,
}

impl CredentialGenerationDefinition {
    pub(crate) const fn new(
        routing_generation: u64,
        authentication_version: u64,
        authentication: CredentialAuthentication,
    ) -> Self {
        Self {
            routing_generation,
            authentication_version,
            authentication,
        }
    }
}

pub(crate) enum CredentialAuthentication {
    ProviderApiKey(Arc<ProviderSecret>),
    OAuth(Arc<OAuthTokenMaterial>),
}

impl CredentialAuthentication {
    pub(crate) const fn provider_api_key(secret: Arc<ProviderSecret>) -> Self {
        Self::ProviderApiKey(secret)
    }

    pub(crate) const fn oauth(token: Arc<OAuthTokenMaterial>) -> Self {
        Self::OAuth(token)
    }

    fn headers(
        &self,
        driver: &dyn ProviderDriver,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        match self {
            Self::ProviderApiKey(secret) => driver.credential_headers(secret),
            Self::OAuth(token) => driver.oauth_credential_headers(token, forwarded),
        }
    }

    fn oauth_token(&self) -> Option<Arc<OAuthTokenMaterial>> {
        match self {
            Self::OAuth(token) => Some(Arc::clone(token)),
            Self::ProviderApiKey(_) => None,
        }
    }
}

pub struct CredentialGenerationRuntime {
    routing_generation: u64,
    authentication_version: u64,
    authentication: CredentialAuthentication,
    health: Arc<CredentialHealthRuntime>,
}

impl CredentialGenerationRuntime {
    pub(crate) fn new(
        definition: CredentialGenerationDefinition,
        scheduler_epoch: Arc<SchedulerEpoch>,
    ) -> Self {
        Self {
            routing_generation: definition.routing_generation,
            authentication_version: definition.authentication_version,
            authentication: definition.authentication,
            health: CredentialHealthRuntime::new(scheduler_epoch),
        }
    }

    #[must_use]
    pub const fn routing_generation(&self) -> u64 {
        self.routing_generation
    }

    #[must_use]
    pub const fn authentication_version(&self) -> u64 {
        self.authentication_version
    }

    pub(crate) fn credential_headers(
        &self,
        driver: &dyn ProviderDriver,
        forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        self.authentication.headers(driver, forwarded)
    }

    pub(crate) fn oauth_token(&self) -> Option<Arc<OAuthTokenMaterial>> {
        self.authentication.oauth_token()
    }

    pub(crate) fn health(&self) -> &Arc<CredentialHealthRuntime> {
        &self.health
    }

    pub(crate) fn matches(&self, definition: &CredentialGenerationDefinition) -> bool {
        self.routing_generation == definition.routing_generation
            && self.authentication_version == definition.authentication_version
    }

    #[cfg(test)]
    pub(crate) fn provider_secret(&self) -> Option<&ProviderSecret> {
        match &self.authentication {
            CredentialAuthentication::ProviderApiKey(secret) => Some(secret),
            CredentialAuthentication::OAuth(_) => None,
        }
    }
}

impl fmt::Debug for CredentialGenerationRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CredentialGenerationRuntime")
            .field("routing_generation", &self.routing_generation)
            .field("authentication_version", &self.authentication_version)
            .field("authentication", &"[REDACTED]")
            .finish()
    }
}
