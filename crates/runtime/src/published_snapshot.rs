use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, GatewayApiKeyConfiguration, GatewayApiKeyId,
    ModelRouteConfiguration, OAuthAccountConfiguration, ProviderCredentialConfiguration,
    ProviderEndpointConfiguration, ProxyConfiguration, ProxyProfile, RoutingCredentialId,
    SettingsConfiguration,
};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::{GatewayApiKeyVerifier, StoredConfiguration};
use any2api_transport::api::TransportProxy;
use arc_swap::ArcSwap;
use tokio::sync::{Mutex, MutexGuard, watch};

use crate::{
    affinity::{AffinityPolicy, AffinityRegistry},
    auxiliary_scheduler::AuxiliaryScheduler,
    credential_auth::CredentialAuthMaterials,
    credential_runtime::CredentialRuntimeBinding,
    health::{HealthBindings, ReliabilityPolicy},
    proxy_auth::ProxyAuthMaterials,
    queue::{QueueCoordinator, QueuePolicy},
    registry::RuntimeRegistry,
    route_tier_cursor::{RouteTierCursorBinding, RouteTierCursorBindings},
    routing_credential::{RoutingCredential, RoutingCredentialSpec, RoutingCredentials},
};

mod oauth;

#[derive(Debug)]
pub struct PublishedSnapshot {
    revision: ConfigRevision,
    proxies: ProxyConfiguration,
    proxy_auth: ProxyAuthMaterials,
    provider_endpoints: ProviderEndpointConfiguration,
    provider_credentials: ProviderCredentialConfiguration,
    oauth_accounts: OAuthAccountConfiguration,
    model_routes: ModelRouteConfiguration,
    gateway_api_keys: GatewayApiKeyConfiguration,
    gateway_api_key_verifier: GatewayApiKeyVerifier,
    settings: SettingsConfiguration,
    affinity_registry: Arc<AffinityRegistry>,
    affinity_policy: AffinityPolicy,
    routing_credentials: RoutingCredentials,
    route_tier_cursors: RouteTierCursorBindings,
    auxiliary_scheduler: Arc<AuxiliaryScheduler>,
    queue_coordinator: Arc<QueueCoordinator>,
    queue_policy: QueuePolicy,
    health: HealthBindings,
    reliability_policy: ReliabilityPolicy,
}

impl PublishedSnapshot {
    #[must_use]
    pub fn new(
        configuration: StoredConfiguration,
        runtime: &RuntimeRegistry,
        providers: &ProviderRegistry,
    ) -> Self {
        let parts = configuration.into_parts();
        let proxy_auth = ProxyAuthMaterials::from_stored(&parts.proxies, parts.proxy_passwords);
        runtime.reconcile_scheduler_settings(parts.settings.scheduler());
        let affinity_policy = AffinityPolicy::from_settings(parts.settings.affinity());
        let queue_policy = QueuePolicy::from_scheduler_settings(parts.settings.scheduler());
        let auth_materials =
            CredentialAuthMaterials::from_stored(parts.provider_credential_secrets);
        let routing_specs = RoutingCredentialSpec::compile(
            &parts.provider_credentials,
            &parts.provider_endpoints,
            &parts.oauth_accounts,
            &parts.proxies,
            auth_materials,
            parts.oauth_account_materials,
            providers,
        );
        let routing_credentials = runtime.reconcile_configuration(routing_specs);
        let oauth_route_tiers = oauth::route_tiers(&parts.model_routes, &routing_credentials);
        let route_tier_cursors =
            runtime.reconcile_route_tier_cursors(&parts.model_routes, &oauth_route_tiers);
        let oauth_endpoints = routing_credentials
            .as_slice()
            .iter()
            .filter(|credential| credential.is_oauth())
            .map(|credential| {
                (
                    credential.endpoint_id(),
                    credential.endpoint_config_version(),
                )
            })
            .collect::<Vec<_>>();
        let health =
            runtime.reconcile_health(&parts.provider_endpoints, &oauth_endpoints, &parts.proxies);
        let reliability_policy = ReliabilityPolicy::from_settings(parts.settings.reliability());
        let auxiliary_scheduler = runtime.auxiliary_scheduler();
        let queue_coordinator = runtime.queue_coordinator();
        let affinity_registry = runtime.affinity_registry();
        Self {
            revision: parts.revision,
            proxies: parts.proxies,
            proxy_auth,
            provider_endpoints: parts.provider_endpoints,
            provider_credentials: parts.provider_credentials,
            oauth_accounts: parts.oauth_accounts,
            model_routes: parts.model_routes,
            gateway_api_keys: parts.gateway_api_keys,
            gateway_api_key_verifier: parts.gateway_api_key_verifier,
            settings: parts.settings,
            affinity_registry,
            affinity_policy,
            routing_credentials,
            route_tier_cursors,
            auxiliary_scheduler,
            queue_coordinator,
            queue_policy,
            health,
            reliability_policy,
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
    pub const fn oauth_accounts(&self) -> &OAuthAccountConfiguration {
        &self.oauth_accounts
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
    pub const fn affinity_policy(&self) -> AffinityPolicy {
        self.affinity_policy
    }

    pub(crate) const fn affinity_registry(&self) -> &Arc<AffinityRegistry> {
        &self.affinity_registry
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
    pub fn credential_runtime(&self, id: RoutingCredentialId) -> Option<&CredentialRuntimeBinding> {
        self.routing_credentials
            .get(id)
            .map(RoutingCredential::binding)
    }

    #[must_use]
    pub fn credential_runtimes(&self) -> &[CredentialRuntimeBinding] {
        self.routing_credentials.bindings()
    }

    pub(crate) fn routing_credentials(&self) -> &[RoutingCredential] {
        self.routing_credentials.as_slice()
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

    pub(crate) const fn reliability_policy(&self) -> ReliabilityPolicy {
        self.reliability_policy
    }

    pub(crate) fn endpoint_health(
        &self,
        id: any2api_domain::ProviderEndpointId,
    ) -> Option<&Arc<crate::health::EndpointHealthRuntime>> {
        self.health.endpoint(id)
    }

    pub(crate) fn proxy_health(
        &self,
        id: any2api_domain::ProxyProfileId,
    ) -> Option<&Arc<crate::health::ProxyHealthRuntime>> {
        self.health.proxy(id)
    }

    #[must_use]
    pub fn resolved_proxy_for_credential(&self, id: CredentialId) -> Option<&ProxyProfile> {
        let credential = self.provider_credentials.get(id)?;
        self.proxies.resolve(credential.proxy_profile_id())
    }

    pub(crate) fn transport_proxy(
        &self,
        id: any2api_domain::ProxyProfileId,
    ) -> Option<TransportProxy<'_>> {
        let profile = self.proxies.get(id)?;
        Some(TransportProxy::new(
            profile,
            self.proxy_auth.credentials_for(profile),
        ))
    }

    pub(crate) fn resolved_transport_proxy_for_credential(
        &self,
        id: CredentialId,
    ) -> Option<TransportProxy<'_>> {
        let profile = self.resolved_proxy_for_credential(id)?;
        Some(TransportProxy::new(
            profile,
            self.proxy_auth.credentials_for(profile),
        ))
    }

    pub(crate) fn resolved_transport_proxy_for_oauth_account(&self) -> Option<TransportProxy<'_>> {
        let profile = self
            .proxies
            .resolve(any2api_domain::ProxyProfileId::DIRECT)?;
        Some(TransportProxy::new(
            profile,
            self.proxy_auth.credentials_for(profile),
        ))
    }
}

#[derive(Debug)]
pub struct SnapshotStore {
    current: ArcSwap<PublishedSnapshot>,
    publish_serial: Mutex<()>,
    revision_sender: watch::Sender<ConfigRevision>,
}

impl SnapshotStore {
    #[must_use]
    pub fn new(initial: PublishedSnapshot) -> Self {
        let (revision_sender, _receiver) = watch::channel(initial.revision());
        Self {
            current: ArcSwap::from_pointee(initial),
            publish_serial: Mutex::new(()),
            revision_sender,
        }
    }

    #[must_use]
    pub fn load(&self) -> Arc<PublishedSnapshot> {
        self.current.load_full()
    }

    pub(crate) async fn acquire_publish(&self) -> MutexGuard<'_, ()> {
        self.publish_serial.lock().await
    }

    pub(crate) fn subscribe_revision(&self) -> watch::Receiver<ConfigRevision> {
        self.revision_sender.subscribe()
    }

    pub(crate) fn replace(&self, next: PublishedSnapshot) -> Arc<PublishedSnapshot> {
        let next = Arc::new(next);
        self.current.store(Arc::clone(&next));
        self.revision_sender.send_replace(next.revision());
        next
    }
}
