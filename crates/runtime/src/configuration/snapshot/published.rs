use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, GatewayApiKeyConfiguration, GatewayApiKeyId,
    ModelRouteConfiguration, OAuthAccountConfiguration, ProviderCredentialConfiguration,
    ProviderEndpointConfiguration, ProxyConfiguration, ProxyProfile, RoutingCredentialId,
    SettingsConfiguration,
};
use any2api_storage::api::GatewayApiKeyVerifier;
use any2api_storage::api::StoredConfiguration;
use any2api_transport::api::TransportProxy;

use super::{PreparedPublishedSnapshot, SnapshotCompileError};
use crate::{
    affinity::{AffinityPolicy, AffinityRegistry},
    credential::CredentialRuntimeBinding,
    health::{HealthBindings, ReliabilityPolicy},
    proxy::ProxyAuthMaterials,
    registry::RuntimeRegistry,
    routing::{
        QueueCoordinator, QueuePolicy, RouteTierCursorBinding, RouteTierCursorBindings,
        RoutingCredential, RoutingCredentials,
    },
};

#[derive(Debug)]
pub struct PublishedSnapshot {
    pub(super) revision: ConfigRevision,
    pub(super) proxies: ProxyConfiguration,
    pub(super) proxy_auth: ProxyAuthMaterials,
    pub(super) provider_endpoints: ProviderEndpointConfiguration,
    pub(super) provider_credentials: ProviderCredentialConfiguration,
    pub(super) oauth_accounts: OAuthAccountConfiguration,
    pub(super) model_routes: ModelRouteConfiguration,
    pub(super) gateway_api_keys: GatewayApiKeyConfiguration,
    pub(super) gateway_api_key_verifier: GatewayApiKeyVerifier,
    pub(super) settings: SettingsConfiguration,
    pub(super) affinity_registry: Arc<AffinityRegistry>,
    pub(super) affinity_policy: AffinityPolicy,
    pub(super) routing_credentials: RoutingCredentials,
    pub(super) route_tier_cursors: RouteTierCursorBindings,
    pub(super) queue_coordinator: Arc<QueueCoordinator>,
    pub(super) queue_policy: QueuePolicy,
    pub(super) health: HealthBindings,
    pub(super) reliability_policy: ReliabilityPolicy,
}

impl PublishedSnapshot {
    pub fn new(
        configuration: StoredConfiguration,
        runtime: &RuntimeRegistry,
        providers: &any2api_provider::api::ProviderRegistry,
    ) -> Result<Self, SnapshotCompileError> {
        PreparedPublishedSnapshot::compile(configuration, providers)
            .map(|prepared| prepared.bind(runtime))
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
    pub fn credential_runtimes(
        &self,
    ) -> impl ExactSizeIterator<Item = &CredentialRuntimeBinding> + '_ {
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
