use std::collections::HashMap;

use any2api_domain::{
    MaxConcurrency, ProtocolDialect, ProviderBaseUrl, ProviderEndpointId, ProviderKind,
    ProxyProfileId, RoutingCredentialId, UpstreamModelName,
};

use crate::credential_runtime::{CredentialGenerationDefinition, CredentialRuntimeBinding};

mod compile;

pub(crate) struct RoutingCredentialSpec {
    id: RoutingCredentialId,
    provider_kind: ProviderKind,
    label: String,
    endpoint_id: ProviderEndpointId,
    endpoint_name: String,
    endpoint_config_version: u64,
    base_url: ProviderBaseUrl,
    ingress_protocol: ProtocolDialect,
    upstream_protocol: ProtocolDialect,
    proxy_id: ProxyProfileId,
    enabled: bool,
    expires_at: Option<i64>,
    endpoint_enabled: bool,
    models: Vec<UpstreamModelName>,
    available_models: Vec<UpstreamModelName>,
    max_concurrency: MaxConcurrency,
    generation: Option<CredentialGenerationDefinition>,
}

impl RoutingCredentialSpec {
    pub(crate) const fn id(&self) -> RoutingCredentialId {
        self.id
    }

    pub(crate) const fn max_concurrency(&self) -> MaxConcurrency {
        self.max_concurrency
    }

    pub(crate) fn take_generation(&mut self) -> CredentialGenerationDefinition {
        self.generation
            .take()
            .expect("routing credential generation is consumed once")
    }

    pub(crate) fn bind(self, binding: CredentialRuntimeBinding) -> RoutingCredential {
        RoutingCredential {
            id: self.id,
            provider_kind: self.provider_kind,
            label: self.label,
            endpoint_id: self.endpoint_id,
            endpoint_name: self.endpoint_name,
            endpoint_config_version: self.endpoint_config_version,
            base_url: self.base_url,
            ingress_protocol: self.ingress_protocol,
            upstream_protocol: self.upstream_protocol,
            proxy_id: self.proxy_id,
            enabled: self.enabled,
            expires_at: self.expires_at,
            endpoint_enabled: self.endpoint_enabled,
            models: self.models,
            available_models: self.available_models,
            binding,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RoutingCredential {
    id: RoutingCredentialId,
    provider_kind: ProviderKind,
    label: String,
    endpoint_id: ProviderEndpointId,
    endpoint_name: String,
    endpoint_config_version: u64,
    base_url: ProviderBaseUrl,
    ingress_protocol: ProtocolDialect,
    upstream_protocol: ProtocolDialect,
    proxy_id: ProxyProfileId,
    enabled: bool,
    expires_at: Option<i64>,
    endpoint_enabled: bool,
    models: Vec<UpstreamModelName>,
    available_models: Vec<UpstreamModelName>,
    binding: CredentialRuntimeBinding,
}

impl RoutingCredential {
    pub(crate) const fn id(&self) -> RoutingCredentialId {
        self.id
    }
    pub(crate) const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }
    pub(crate) fn label(&self) -> &str {
        &self.label
    }
    pub(crate) const fn endpoint_id(&self) -> ProviderEndpointId {
        self.endpoint_id
    }
    pub(crate) fn endpoint_name(&self) -> &str {
        &self.endpoint_name
    }
    pub(crate) const fn endpoint_config_version(&self) -> u64 {
        self.endpoint_config_version
    }
    pub(crate) const fn base_url(&self) -> &ProviderBaseUrl {
        &self.base_url
    }
    pub(crate) const fn ingress_protocol(&self) -> ProtocolDialect {
        self.ingress_protocol
    }
    pub(crate) const fn upstream_protocol(&self) -> ProtocolDialect {
        self.upstream_protocol
    }
    pub(crate) const fn proxy_id(&self) -> ProxyProfileId {
        self.proxy_id
    }
    pub(crate) const fn enabled(&self) -> bool {
        self.enabled
    }
    pub(crate) fn authentication_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| expires_at <= unix_now())
    }
    pub(crate) fn routable(&self) -> bool {
        self.enabled && self.endpoint_enabled && !self.authentication_expired()
    }
    pub(crate) const fn endpoint_enabled(&self) -> bool {
        self.endpoint_enabled
    }
    pub(crate) fn models(&self) -> &[UpstreamModelName] {
        &self.models
    }
    pub(crate) fn available_models(&self) -> &[UpstreamModelName] {
        &self.available_models
    }
    pub(crate) fn supports_model(&self, model: &UpstreamModelName) -> bool {
        self.models.binary_search(model).is_ok()
    }
    pub(crate) const fn binding(&self) -> &CredentialRuntimeBinding {
        &self.binding
    }
    pub(crate) const fn is_oauth(&self) -> bool {
        self.id.oauth_account_id().is_some()
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(i64::MAX)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct RoutingCredentials {
    ordered: Vec<RoutingCredential>,
    by_id: HashMap<RoutingCredentialId, usize>,
    bindings: Vec<CredentialRuntimeBinding>,
}

impl RoutingCredentials {
    pub(crate) fn new(ordered: Vec<RoutingCredential>) -> Self {
        let by_id = ordered
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id(), i))
            .collect();
        let bindings = ordered.iter().map(|item| item.binding().clone()).collect();
        Self {
            ordered,
            by_id,
            bindings,
        }
    }
    pub(crate) fn get(&self, id: RoutingCredentialId) -> Option<&RoutingCredential> {
        self.by_id.get(&id).map(|index| &self.ordered[*index])
    }
    pub(crate) fn as_slice(&self) -> &[RoutingCredential] {
        &self.ordered
    }
    pub(crate) fn bindings(&self) -> &[CredentialRuntimeBinding] {
        &self.bindings
    }
}
