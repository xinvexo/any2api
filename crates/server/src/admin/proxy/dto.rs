use any2api_domain::{
    ConfigRevision, ProviderEndpointId, ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile,
    ProxyProfileId,
};
use any2api_runtime::api::{
    ProxyPasswordSecret, ProxyTestOutcome, ProxyTestResult, PublishedSnapshot,
};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Serialize)]
pub(crate) struct ProxyCollectionResponse {
    config_revision: u64,
    global_proxy_id: ProxyProfileId,
    items: Vec<ProxyResponse>,
}

impl ProxyCollectionResponse {
    pub(crate) fn from_snapshot(snapshot: &PublishedSnapshot) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            global_proxy_id: snapshot.proxies().global_proxy_id(),
            items: snapshot
                .proxies()
                .profiles()
                .iter()
                .map(ProxyResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProxyResponse {
    id: ProxyProfileId,
    name: String,
    kind: ProxyKind,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password_configured: bool,
    authentication_version: u64,
    enabled: bool,
    built_in: bool,
    config_version: u64,
}

impl From<&ProxyProfile> for ProxyResponse {
    fn from(profile: &ProxyProfile) -> Self {
        Self {
            id: profile.id(),
            name: profile.name().to_owned(),
            kind: profile.kind(),
            host: profile.address().map(|address| address.host().to_owned()),
            port: profile.address().map(ProxyAddress::port),
            username: profile
                .authentication()
                .map(|authentication| authentication.username().to_owned()),
            password_configured: profile.authentication().is_some(),
            authentication_version: profile.authentication_version(),
            enabled: profile.enabled(),
            built_in: profile.is_built_in(),
            config_version: profile.config_version(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProxyWriteRequest {
    expected_revision: u64,
    name: String,
    kind: ProxyKind,
    host: String,
    port: u16,
    enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProxyAuthenticationRequest {
    expected_revision: u64,
    username: String,
    password: String,
}

impl ProxyAuthenticationRequest {
    pub(crate) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, String, ProxyPasswordSecret), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            self.username,
            ProxyPasswordSecret::new(self.password),
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProxyTestRequest {
    provider_endpoint_id: ProviderEndpointId,
}

impl ProxyTestRequest {
    pub(crate) const fn provider_endpoint_id(&self) -> ProviderEndpointId {
        self.provider_endpoint_id
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ProxyTestResponse {
    config_revision: u64,
    proxy_config_version: u64,
    provider_endpoint_config_version: u64,
    proxy_id: ProxyProfileId,
    provider_endpoint_id: ProviderEndpointId,
    reachable: bool,
    status_code: Option<u16>,
    latency_ms: u64,
    error_stage: Option<&'static str>,
    failure_scope: Option<&'static str>,
}

impl From<ProxyTestResult> for ProxyTestResponse {
    fn from(result: ProxyTestResult) -> Self {
        let (reachable, status_code, error_stage, failure_scope) = match result.outcome() {
            ProxyTestOutcome::Reachable { status_code } => (true, Some(status_code), None, None),
            ProxyTestOutcome::Failed { stage, scope } => {
                (false, None, Some(stage.as_str()), Some(scope.as_str()))
            }
        };
        Self {
            config_revision: result.config_revision().get(),
            proxy_config_version: result.proxy_config_version(),
            provider_endpoint_config_version: result.provider_endpoint_config_version(),
            proxy_id: result.proxy_id(),
            provider_endpoint_id: result.provider_endpoint_id(),
            reachable,
            status_code,
            latency_ms: result.latency_ms(),
            error_stage,
            failure_scope,
        }
    }
}

impl ProxyWriteRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, ProxyDraft), AdminApiError> {
        let revision = parse_revision(self.expected_revision)?;
        let address = ProxyAddress::new(self.host, self.port)
            .map_err(|error| AdminApiError::invalid_request(error.to_string()))?;
        let draft = ProxyDraft::new(self.name, self.kind, address, self.enabled)
            .map_err(|error| AdminApiError::invalid_request(error.to_string()))?;

        Ok((revision, draft))
    }
}
