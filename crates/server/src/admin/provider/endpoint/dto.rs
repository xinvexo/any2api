use any2api_domain::{
    ConfigRevision, ProtocolDialect, ProtocolOperation, ProviderEndpoint, ProviderEndpointDraft,
    ProviderEndpointId, ProviderKind,
};
use any2api_runtime::api::{
    ConfigurationCapabilities, ProtocolBridgeCapabilities, ProviderProtocolOptions,
    ProviderUpstreamProtocolOption, PublishedSnapshot,
};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Serialize)]
pub(crate) struct ProviderEndpointCollectionResponse {
    config_revision: u64,
    items: Vec<ProviderEndpointResponse>,
    protocol_options: Vec<ProviderProtocolOptionsResponse>,
}

impl ProviderEndpointCollectionResponse {
    pub(crate) fn from_snapshot(
        snapshot: &PublishedSnapshot,
        capabilities: &ConfigurationCapabilities,
    ) -> Self {
        let protocol_options = ProviderKind::ALL
            .into_iter()
            .flat_map(|provider| capabilities.provider_protocol_options(provider))
            .map(ProviderProtocolOptionsResponse::from)
            .collect();
        Self {
            config_revision: snapshot.revision().get(),
            items: snapshot
                .provider_endpoints()
                .endpoints()
                .iter()
                .map(ProviderEndpointResponse::from)
                .collect(),
            protocol_options,
        }
    }
}

#[derive(Debug, Serialize)]
struct ProviderProtocolOptionsResponse {
    provider_kind: ProviderKind,
    accepted_protocol: ProtocolDialect,
    upstream_options: Vec<ProviderUpstreamProtocolOptionResponse>,
}

impl From<ProviderProtocolOptions> for ProviderProtocolOptionsResponse {
    fn from(options: ProviderProtocolOptions) -> Self {
        Self {
            provider_kind: options.provider_kind,
            accepted_protocol: options.accepted_protocol,
            upstream_options: options
                .upstream_options
                .into_iter()
                .map(ProviderUpstreamProtocolOptionResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProviderUpstreamProtocolOptionResponse {
    protocol: ProtocolDialect,
    fidelity: &'static str,
    operations: Vec<ProtocolOperation>,
    bridge: Option<ProtocolBridgeCapabilitiesResponse>,
}

impl From<ProviderUpstreamProtocolOption> for ProviderUpstreamProtocolOptionResponse {
    fn from(option: ProviderUpstreamProtocolOption) -> Self {
        Self {
            protocol: option.protocol,
            fidelity: option.fidelity.as_str(),
            operations: option.operations,
            bridge: option.bridge.map(ProtocolBridgeCapabilitiesResponse::from),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProtocolBridgeCapabilitiesResponse {
    contract_id: &'static str,
    request_fields: Vec<BridgeRequestFieldCapabilityResponse>,
    tool_types: &'static [&'static str],
    limitations: Vec<BridgeLimitationResponse>,
}

impl From<&'static ProtocolBridgeCapabilities> for ProtocolBridgeCapabilitiesResponse {
    fn from(capabilities: &'static ProtocolBridgeCapabilities) -> Self {
        Self {
            contract_id: capabilities.contract_id,
            request_fields: capabilities
                .request_fields
                .iter()
                .map(|field| BridgeRequestFieldCapabilityResponse {
                    path: field.path,
                    behavior: field.behavior.as_str(),
                })
                .collect(),
            tool_types: capabilities.tool_types,
            limitations: capabilities
                .limitations
                .iter()
                .map(|limitation| BridgeLimitationResponse {
                    code: limitation.code,
                    description: limitation.description,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct BridgeRequestFieldCapabilityResponse {
    path: &'static str,
    behavior: &'static str,
}

#[derive(Debug, Serialize)]
struct BridgeLimitationResponse {
    code: &'static str,
    description: &'static str,
}

#[derive(Debug, Serialize)]
struct ProviderEndpointResponse {
    id: ProviderEndpointId,
    name: String,
    provider_kind: ProviderKind,
    base_url: String,
    protocol_dialect: ProtocolDialect,
    upstream_protocol_dialect: Option<ProtocolDialect>,
    enabled: bool,
    config_version: u64,
}

impl From<&ProviderEndpoint> for ProviderEndpointResponse {
    fn from(endpoint: &ProviderEndpoint) -> Self {
        Self {
            id: endpoint.id(),
            name: endpoint.name().to_owned(),
            provider_kind: endpoint.provider_kind(),
            base_url: endpoint.base_url().as_str().to_owned(),
            protocol_dialect: endpoint.protocol_dialect(),
            upstream_protocol_dialect: endpoint.upstream_protocol_dialect(),
            enabled: endpoint.enabled(),
            config_version: endpoint.config_version(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderEndpointWriteRequest {
    expected_revision: u64,
    expected_config_version: Option<u64>,
    name: String,
    provider_kind: ProviderKind,
    base_url: String,
    protocol_dialect: ProtocolDialect,
    upstream_protocol_dialect: Option<ProtocolDialect>,
    enabled: bool,
}

impl ProviderEndpointWriteRequest {
    pub(crate) fn into_create_domain(
        self,
    ) -> Result<(ConfigRevision, ProviderEndpointDraft), AdminApiError> {
        let (revision, expected_config_version, draft) = self.into_parts()?;
        if expected_config_version.is_some() {
            return Err(AdminApiError::invalid_request(
                "expected_config_version is only valid for updates",
            ));
        }
        Ok((revision, draft))
    }

    pub(crate) fn into_update_domain(
        self,
    ) -> Result<(ConfigRevision, u64, ProviderEndpointDraft), AdminApiError> {
        let (revision, expected_config_version, draft) = self.into_parts()?;
        let expected_config_version = expected_config_version
            .filter(|value| *value > 0)
            .ok_or_else(|| {
                AdminApiError::invalid_request("expected_config_version is required for updates")
            })?;
        Ok((revision, expected_config_version, draft))
    }

    fn into_parts(
        self,
    ) -> Result<(ConfigRevision, Option<u64>, ProviderEndpointDraft), AdminApiError> {
        let revision = parse_revision(self.expected_revision)?;
        let expected_config_version = self.expected_config_version;
        let draft = ProviderEndpointDraft::with_upstream_protocol(
            self.name,
            self.provider_kind,
            self.base_url,
            self.protocol_dialect,
            self.upstream_protocol_dialect,
            self.enabled,
        )
        .map_err(|error| AdminApiError::invalid_provider_endpoint(error.to_string()))?;
        Ok((revision, expected_config_version, draft))
    }
}
