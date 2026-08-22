use any2api_domain::{ConfigRevision, ConfigurationCore};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::StoredConfiguration;

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
    core: ConfigurationCore,
    proxy_auth: ProxyAuthMaterials,
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
        let core = ConfigurationCore::from_parts(parts.core);
        for endpoint in core.provider_endpoints().endpoints() {
            if providers.get(endpoint.provider_kind()).is_none() {
                return Err(RoutingCredentialCompileError::MissingProviderDriver(
                    endpoint.provider_kind(),
                )
                .into());
            }
        }
        let proxy_auth = ProxyAuthMaterials::compile(core.proxies(), parts.proxy_passwords)?;
        let credential_auth =
            CredentialAuthMaterials::from_stored(parts.provider_credential_secrets)
                .map_err(RoutingCredentialCompileError::from)?;
        let routing_specs = RoutingCredentialSpec::compile(
            core.provider_credentials(),
            core.provider_endpoints(),
            core.oauth_accounts(),
            core.proxies(),
            credential_auth,
            parts.oauth_account_materials,
            providers,
        )?;
        let affinity_policy = AffinityPolicy::from_settings(core.settings().affinity());
        let queue_policy = QueuePolicy::from_scheduler_settings(core.settings().scheduler());
        let reliability_policy = ReliabilityPolicy::from_settings(core.settings().reliability());
        Ok(Self {
            core,
            proxy_auth,
            affinity_policy,
            routing_specs,
            queue_policy,
            reliability_policy,
        })
    }

    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.core.revision()
    }

    #[must_use]
    pub fn bind(self, runtime: &RuntimeRegistry) -> PublishedSnapshot {
        let routing_credentials = runtime.reconcile_configuration(self.routing_specs);
        let oauth_route_tiers = oauth::route_tiers(self.core.model_routes(), &routing_credentials);
        let route_tier_cursors =
            runtime.reconcile_route_tier_cursors(self.core.model_routes(), &oauth_route_tiers);
        let gateway_api_key_index = self
            .core
            .gateway_api_keys()
            .keys()
            .iter()
            .filter(|key| key.is_active())
            .map(|key| (*key.token_hash(), key.id()))
            .collect();
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
        let health = runtime.reconcile_health(
            self.core.provider_endpoints(),
            &oauth_endpoints,
            self.core.proxies(),
            self.core.model_routes(),
            &routing_credentials,
        );
        PublishedSnapshot {
            core: self.core,
            proxy_auth: self.proxy_auth,
            gateway_api_key_index,
            affinity_registry: runtime.affinity_registry(),
            affinity_policy: self.affinity_policy,
            routing_credentials,
            route_tier_cursors,
            queue_coordinator: runtime.queue_coordinator(),
            queue_policy: self.queue_policy,
            health,
            reliability_policy: self.reliability_policy,
            route_candidate_cache: crate::routing::RouteCandidateCache::default(),
        }
    }
}
