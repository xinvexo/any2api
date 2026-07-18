use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, ProviderCredentialConfiguration, ProviderEndpointConfiguration,
    ProxyConfiguration, ProxyProfile,
};
use any2api_storage::api::StoredConfiguration;
use arc_swap::ArcSwap;
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    credential_auth::CredentialAuthMaterials,
    credential_runtime::{CredentialRuntimeBinding, CredentialRuntimeBindings},
    registry::RuntimeRegistry,
};

#[derive(Debug)]
pub struct PublishedSnapshot {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
    provider_endpoints: ProviderEndpointConfiguration,
    provider_credentials: ProviderCredentialConfiguration,
    credential_runtimes: CredentialRuntimeBindings,
}

impl PublishedSnapshot {
    #[must_use]
    pub fn new(configuration: StoredConfiguration, runtime: &RuntimeRegistry) -> Self {
        let parts = configuration.into_parts();
        let auth_materials =
            CredentialAuthMaterials::from_stored(parts.provider_credential_secrets);
        let credential_runtimes =
            runtime.reconcile_configuration(&parts.provider_credentials, auth_materials);
        Self {
            revision: parts.revision,
            proxies: parts.proxies,
            provider_endpoints: parts.provider_endpoints,
            provider_credentials: parts.provider_credentials,
            credential_runtimes,
        }
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    #[must_use]
    pub const fn proxies(&self) -> &ProxyConfiguration {
        &self.proxies
    }

    #[must_use]
    pub const fn provider_endpoints(&self) -> &ProviderEndpointConfiguration {
        &self.provider_endpoints
    }

    #[must_use]
    pub const fn provider_credentials(&self) -> &ProviderCredentialConfiguration {
        &self.provider_credentials
    }

    #[must_use]
    pub fn credential_runtime(&self, id: CredentialId) -> Option<&CredentialRuntimeBinding> {
        self.credential_runtimes.get(id)
    }

    #[must_use]
    pub fn credential_runtimes(&self) -> &[CredentialRuntimeBinding] {
        self.credential_runtimes.as_slice()
    }

    #[must_use]
    pub fn resolved_proxy_for_credential(&self, id: CredentialId) -> Option<&ProxyProfile> {
        let credential = self.provider_credentials.get(id)?;
        self.proxies.resolve(credential.proxy_profile_id())
    }
}

#[derive(Debug)]
pub struct SnapshotStore {
    current: ArcSwap<PublishedSnapshot>,
    publish_serial: Mutex<()>,
}

impl SnapshotStore {
    #[must_use]
    pub fn new(initial: PublishedSnapshot) -> Self {
        Self {
            current: ArcSwap::from_pointee(initial),
            publish_serial: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn load(&self) -> Arc<PublishedSnapshot> {
        self.current.load_full()
    }

    pub(crate) async fn acquire_publish(&self) -> MutexGuard<'_, ()> {
        self.publish_serial.lock().await
    }

    pub(crate) fn replace(&self, next: PublishedSnapshot) -> Arc<PublishedSnapshot> {
        let next = Arc::new(next);
        self.current.store(Arc::clone(&next));
        next
    }
}
