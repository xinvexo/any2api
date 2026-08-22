use std::collections::HashMap;
use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, ConfigurationCore, CredentialId, GatewayApiKeyConfiguration, GatewayApiKeyId,
    GatewayApiKeyVerifier, ModelRoute, ModelRouteConfiguration, OAuthAccountConfiguration,
    OAuthAccountId, OAuthProxySelection, ProviderCredentialConfiguration,
    ProviderEndpointConfiguration, ProxyConfiguration, ProxyProfile, RoutingCredentialId,
    SettingsConfiguration, validate_gateway_token,
};
use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::StoredConfiguration;
use any2api_transport::api::TransportProxy;

use super::{PreparedPublishedSnapshot, SnapshotCompileError};
use crate::{
    affinity::{AffinityPolicy, AffinityRegistry},
    credential::CredentialRuntimeBinding,
    health::{CandidatePathKey, EgressPathKey, HealthBindings, ReliabilityPolicy},
    proxy::ProxyAuthMaterials,
    registry::RuntimeRegistry,
    routing::{
        BreakerStateCounts, CandidateRequirements, OAuthRoute, QueueCoordinator, QueuePolicy,
        RouteCandidateCache, RouteCandidateTiers, RouteTierCursorBinding, RouteTierCursorBindings,
        RoutingCredential, RoutingCredentials, build_oauth_route_candidates,
        build_route_candidates,
    },
};

#[derive(Debug)]
pub struct PublishedSnapshot {
    pub(super) core: ConfigurationCore,
    pub(super) proxy_auth: ProxyAuthMaterials,
    pub(super) gateway_api_key_index: HashMap<[u8; 32], GatewayApiKeyId>,
    pub(super) affinity_registry: Arc<AffinityRegistry>,
    pub(super) affinity_policy: AffinityPolicy,
    pub(super) routing_credentials: RoutingCredentials,
    pub(super) route_tier_cursors: RouteTierCursorBindings,
    pub(super) queue_coordinator: Arc<QueueCoordinator>,
    pub(super) queue_policy: QueuePolicy,
    pub(super) health: HealthBindings,
    pub(super) reliability_policy: ReliabilityPolicy,
    pub(super) route_candidate_cache: RouteCandidateCache,
}

/// Opaque, non-secret evidence that ingress authentication succeeded against
/// one PublishedSnapshot. External callers can forward it but cannot forge a
/// key id or token version pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GatewayApiKeyAuthProof {
    id: GatewayApiKeyId,
    token_version: u64,
}

impl GatewayApiKeyAuthProof {
    #[must_use]
    pub const fn id(self) -> GatewayApiKeyId {
        self.id
    }

    pub(crate) const fn token_version(self) -> u64 {
        self.token_version
    }
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
        self.core.revision()
    }

    #[must_use]
    pub const fn proxies(&self) -> &ProxyConfiguration {
        self.core.proxies()
    }

    #[must_use]
    pub const fn provider_endpoints(&self) -> &ProviderEndpointConfiguration {
        self.core.provider_endpoints()
    }

    #[must_use]
    pub const fn provider_credentials(&self) -> &ProviderCredentialConfiguration {
        self.core.provider_credentials()
    }

    #[must_use]
    pub const fn oauth_accounts(&self) -> &OAuthAccountConfiguration {
        self.core.oauth_accounts()
    }

    #[must_use]
    pub const fn model_routes(&self) -> &ModelRouteConfiguration {
        self.core.model_routes()
    }

    #[must_use]
    pub const fn gateway_api_keys(&self) -> &GatewayApiKeyConfiguration {
        self.core.gateway_api_keys()
    }

    #[must_use]
    pub const fn settings(&self) -> &SettingsConfiguration {
        self.core.settings()
    }

    #[must_use]
    pub const fn affinity_policy(&self) -> AffinityPolicy {
        self.affinity_policy
    }

    pub(crate) const fn affinity_registry(&self) -> &Arc<AffinityRegistry> {
        &self.affinity_registry
    }

    #[must_use]
    pub fn authenticate_gateway_api_key(&self, token: &str) -> Option<GatewayApiKeyAuthProof> {
        validate_gateway_token(token).ok()?;
        let verifier = GatewayApiKeyVerifier::new();
        let digest = verifier.hash(token.as_bytes());
        let id = *self.gateway_api_key_index.get(&digest)?;
        self.gateway_api_keys()
            .get(id)
            .filter(|key| key.is_active() && verifier.verify_digest(&digest, key.token_hash()))
            .map(|key| GatewayApiKeyAuthProof {
                id: key.id(),
                token_version: key.token_version(),
            })
    }

    pub(crate) fn route_candidates(
        &self,
        route: &ModelRoute,
        protocols: &ProtocolRegistry,
        providers: &ProviderRegistry,
        requirements: CandidateRequirements,
    ) -> Arc<RouteCandidateTiers> {
        self.route_candidate_cache
            .get_or_build(route.id(), requirements, || {
                build_route_candidates(self, route, protocols, providers, requirements)
            })
    }

    pub(crate) fn oauth_route_candidates(
        &self,
        route: OAuthRoute<'_>,
        protocols: &ProtocolRegistry,
        providers: &ProviderRegistry,
        requirements: CandidateRequirements,
    ) -> Arc<RouteCandidateTiers> {
        self.route_candidate_cache
            .get_or_build(route.route_id(), requirements, || {
                build_oauth_route_candidates(self, route, protocols, providers, requirements)
            })
    }

    #[cfg(test)]
    pub(crate) fn route_candidate_cache_entry_count(&self) -> usize {
        self.route_candidate_cache.entry_count()
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

    pub(crate) fn breaker_state_counts(&self) -> BreakerStateCounts {
        self.health.breaker_state_counts()
    }

    pub(crate) fn egress_path_health(
        &self,
        key: EgressPathKey,
    ) -> Arc<crate::health::EndpointHealthRuntime> {
        self.health.egress_path(key)
    }

    pub(crate) fn candidate_path_health(
        &self,
        key: CandidatePathKey,
    ) -> Arc<crate::health::EndpointHealthRuntime> {
        self.health.candidate_path(key)
    }

    #[must_use]
    pub fn resolved_proxy_for_credential(&self, id: CredentialId) -> Option<&ProxyProfile> {
        let credential = self.provider_credentials().get(id)?;
        self.proxies().get(credential.proxy_profile_id())
    }

    pub(crate) fn transport_proxy(
        &self,
        id: any2api_domain::ProxyProfileId,
    ) -> Option<TransportProxy<'_>> {
        let profile = self.proxies().get(id)?;
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

    pub(crate) fn resolved_transport_proxy_for_oauth_selection(
        &self,
        selection: OAuthProxySelection,
    ) -> Option<TransportProxy<'_>> {
        let profile = self.proxies().resolve_oauth(selection)?;
        if !profile.enabled() {
            return None;
        }
        Some(TransportProxy::new(
            profile,
            self.proxy_auth.credentials_for(profile),
        ))
    }

    pub(crate) fn resolved_transport_proxy_for_oauth_account(
        &self,
        id: OAuthAccountId,
    ) -> Option<TransportProxy<'_>> {
        let selection = self.oauth_accounts().get(id)?.proxy_selection();
        self.resolved_transport_proxy_for_oauth_selection(selection)
    }
}
