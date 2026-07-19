use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, GatewayApiKeyConfiguration, GatewayApiKeyId,
    ModelRouteConfiguration, ProviderCredentialConfiguration, ProviderEndpointConfiguration,
    ProxyConfiguration, ProxyProfile, SettingsConfiguration,
};
use any2api_storage::api::{GatewayApiKeyVerifier, StoredConfiguration};
use arc_swap::ArcSwap;
use tokio::sync::{Mutex, MutexGuard};

use crate::{
    auxiliary_scheduler::AuxiliaryScheduler,
    credential_auth::CredentialAuthMaterials,
    credential_runtime::{CredentialRuntimeBinding, CredentialRuntimeBindings},
    queue::{QueueCoordinator, QueuePolicy},
    registry::RuntimeRegistry,
    route_tier_cursor::{RouteTierCursorBinding, RouteTierCursorBindings},
};

#[derive(Debug)]
pub struct PublishedSnapshot {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
    provider_endpoints: ProviderEndpointConfiguration,
    provider_credentials: ProviderCredentialConfiguration,
    model_routes: ModelRouteConfiguration,
    gateway_api_keys: GatewayApiKeyConfiguration,
    gateway_api_key_verifier: GatewayApiKeyVerifier,
    settings: SettingsConfiguration,
    credential_runtimes: CredentialRuntimeBindings,
    route_tier_cursors: RouteTierCursorBindings,
    auxiliary_scheduler: Arc<AuxiliaryScheduler>,
    queue_coordinator: Arc<QueueCoordinator>,
    queue_policy: QueuePolicy,
}

impl PublishedSnapshot {
    #[must_use]
    pub fn new(configuration: StoredConfiguration, runtime: &RuntimeRegistry) -> Self {
        let parts = configuration.into_parts();
        runtime.reconcile_scheduler_settings(parts.settings.scheduler());
        let queue_policy = QueuePolicy::from_scheduler_settings(parts.settings.scheduler());
        let auth_materials =
            CredentialAuthMaterials::from_stored(parts.provider_credential_secrets);
        let credential_runtimes =
            runtime.reconcile_configuration(&parts.provider_credentials, auth_materials);
        let route_tier_cursors = runtime.reconcile_route_tier_cursors(&parts.model_routes);
        let auxiliary_scheduler = runtime.auxiliary_scheduler();
        let queue_coordinator = runtime.queue_coordinator();
        Self {
            revision: parts.revision,
            proxies: parts.proxies,
            provider_endpoints: parts.provider_endpoints,
            provider_credentials: parts.provider_credentials,
            model_routes: parts.model_routes,
            gateway_api_keys: parts.gateway_api_keys,
            gateway_api_key_verifier: parts.gateway_api_key_verifier,
            settings: parts.settings,
            credential_runtimes,
            route_tier_cursors,
            auxiliary_scheduler,
            queue_coordinator,
            queue_policy,
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
    pub const fn model_routes(&self) -> &ModelRouteConfiguration {
        &self.model_routes
    }

    #[must_use]
    pub const fn gateway_api_keys(&self) -> &GatewayApiKeyConfiguration {
        &self.gateway_api_keys
    }

    #[must_use]
    pub const fn settings(&self) -> &SettingsConfiguration {
        &self.settings
    }

    #[must_use]
    pub fn authenticate_gateway_api_key(&self, token: &str) -> Option<GatewayApiKeyId> {
        self.gateway_api_keys
            .keys()
            .iter()
            .find(|key| {
                key.is_active()
                    && self
                        .gateway_api_key_verifier
                        .verify(token.as_bytes(), key.token_hash())
            })
            .map(|key| key.id())
    }

    #[must_use]
    pub fn credential_runtime(&self, id: CredentialId) -> Option<&CredentialRuntimeBinding> {
        self.credential_runtimes.get(id)
    }

    #[must_use]
    pub fn credential_runtimes(&self) -> &[CredentialRuntimeBinding] {
        self.credential_runtimes.as_slice()
    }

    pub(crate) fn route_tier_cursor(
        &self,
        route_id: any2api_domain::ModelRouteId,
        tier: any2api_domain::FallbackTier,
    ) -> Option<&RouteTierCursorBinding> {
        self.route_tier_cursors.get(route_id, tier)
    }

    pub(crate) const fn auxiliary_scheduler(&self) -> &Arc<AuxiliaryScheduler> {
        &self.auxiliary_scheduler
    }

    #[must_use]
    pub(crate) const fn queue_policy(&self) -> QueuePolicy {
        self.queue_policy
    }

    pub(crate) const fn queue_coordinator(&self) -> &Arc<QueueCoordinator> {
        &self.queue_coordinator
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
