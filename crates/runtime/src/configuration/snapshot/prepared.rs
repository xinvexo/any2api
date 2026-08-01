use any2api_domain::{
    ConfigRevision, GatewayApiKeyConfiguration, ModelRouteConfiguration, OAuthAccountConfiguration,
    ProviderCredentialConfiguration, ProviderEndpointConfiguration, ProxyConfiguration,
    SettingsConfiguration,
};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::{GatewayApiKeyVerifier, StoredConfiguration};

use super::{PublishedSnapshot, SnapshotCompileError, oauth};
use crate::{
    affinity::AffinityPolicy,
    credential::CredentialAuthMaterials,
    health::ReliabilityPolicy,
    proxy::ProxyAuthMaterials,
    registry::RuntimeRegistry,
    routing::{QueuePolicy, RoutingCredentialCompileError, RoutingCredentialSpec},
};

pub struct PreparedPublishedSnapshot {
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
    affinity_policy: AffinityPolicy,
    routing_specs: Vec<RoutingCredentialSpec>,
    queue_policy: QueuePolicy,
    reliability_policy: ReliabilityPolicy,
}

impl PreparedPublishedSnapshot {
    pub fn compile(
        configuration: StoredConfiguration,
        providers: &ProviderRegistry,
    ) -> Result<Self, SnapshotCompileError> {
        let parts = configuration.into_parts();
        for endpoint in parts.provider_endpoints.endpoints() {
            if providers.get(endpoint.provider_kind()).is_none() {
                return Err(RoutingCredentialCompileError::MissingProviderDriver(
                    endpoint.provider_kind(),
                )
                .into());
            }
        }
        let proxy_auth = ProxyAuthMaterials::compile(&parts.proxies, parts.proxy_passwords)?;
        let credential_auth =
            CredentialAuthMaterials::from_stored(parts.provider_credential_secrets)
                .map_err(RoutingCredentialCompileError::from)?;
        let routing_specs = RoutingCredentialSpec::compile(
            &parts.provider_credentials,
            &parts.provider_endpoints,
            &parts.oauth_accounts,
            &parts.proxies,
            credential_auth,
            parts.oauth_account_materials,
            providers,
        )?;
        let affinity_policy = AffinityPolicy::from_settings(parts.settings.affinity());
        let queue_policy = QueuePolicy::from_scheduler_settings(parts.settings.scheduler());
        let reliability_policy = ReliabilityPolicy::from_settings(parts.settings.reliability());
        Ok(Self {
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
            affinity_policy,
            routing_specs,
            queue_policy,
            reliability_policy,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    #[must_use]
    pub fn bind(self, runtime: &RuntimeRegistry) -> PublishedSnapshot {
        let routing_credentials = runtime.reconcile_configuration(self.routing_specs);
        let oauth_route_tiers = oauth::route_tiers(&self.model_routes, &routing_credentials);
        let route_tier_cursors =
            runtime.reconcile_route_tier_cursors(&self.model_routes, &oauth_route_tiers);
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
            runtime.reconcile_health(&self.provider_endpoints, &oauth_endpoints, &self.proxies);
        PublishedSnapshot {
            revision: self.revision,
            proxies: self.proxies,
            proxy_auth: self.proxy_auth,
            provider_endpoints: self.provider_endpoints,
            provider_credentials: self.provider_credentials,
            oauth_accounts: self.oauth_accounts,
            model_routes: self.model_routes,
            gateway_api_keys: self.gateway_api_keys,
            gateway_api_key_verifier: self.gateway_api_key_verifier,
            settings: self.settings,
            affinity_registry: runtime.affinity_registry(),
            affinity_policy: self.affinity_policy,
            routing_credentials,
            route_tier_cursors,
            queue_coordinator: runtime.queue_coordinator(),
            queue_policy: self.queue_policy,
            health,
            reliability_policy: self.reliability_policy,
        }
    }
}
