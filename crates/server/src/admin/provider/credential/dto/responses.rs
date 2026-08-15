use any2api_domain::{
    CredentialId, CredentialKind, ProviderCredential, ProviderEndpointId, ProxyProfileId,
    RequestsPerMinute, RoutingCredentialId,
};
use any2api_runtime::api::{
    ProviderCredentialTestOutcome, ProviderCredentialTestResult, PublishedSnapshot,
    UpstreamCredentialUsageSummary,
};
use serde::Serialize;

use crate::admin::credential_runtime::CredentialRuntimeResponse;
use crate::admin::{request_usage::RequestUsageResponse, upstream_usage};

#[derive(Serialize)]
pub(crate) struct ProviderCredentialCollectionResponse {
    config_revision: u64,
    provider_endpoint_id: ProviderEndpointId,
    items: Vec<ProviderCredentialResponse>,
}

impl ProviderCredentialCollectionResponse {
    pub(crate) fn from_snapshot(
        snapshot: &PublishedSnapshot,
        endpoint_id: ProviderEndpointId,
        usage: &[UpstreamCredentialUsageSummary],
    ) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            provider_endpoint_id: endpoint_id,
            items: snapshot
                .provider_credentials()
                .for_endpoint(endpoint_id)
                .map(|credential| ProviderCredentialResponse::new(snapshot, credential, usage))
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct ProviderCredentialResponse {
    id: CredentialId,
    provider_endpoint_id: ProviderEndpointId,
    label: String,
    credential_kind: CredentialKind,
    fingerprint: String,
    secret_tail: Option<String>,
    proxy_profile_id: ProxyProfileId,
    requests_per_minute: Option<u32>,
    enabled: bool,
    secret_version: u64,
    credential_generation: u64,
    config_version: u64,
    models: Vec<ProviderCredentialModelResponse>,
    runtime: CredentialRuntimeResponse,
    usage: RequestUsageResponse,
}

#[derive(Serialize)]
struct ProviderCredentialModelResponse {
    upstream_model: String,
    public_model: Option<String>,
}

impl ProviderCredentialResponse {
    fn new(
        snapshot: &PublishedSnapshot,
        credential: &ProviderCredential,
        usage: &[UpstreamCredentialUsageSummary],
    ) -> Self {
        let runtime = snapshot
            .credential_runtime_observation(RoutingCredentialId::provider_credential(
                credential.id(),
            ))
            .expect("published provider credential has runtime observation");
        Self {
            id: credential.id(),
            provider_endpoint_id: credential.provider_endpoint_id(),
            label: credential.label().to_owned(),
            credential_kind: credential.credential_kind(),
            fingerprint: credential.fingerprint().display(),
            secret_tail: credential.fingerprint().tail().map(str::to_owned),
            proxy_profile_id: credential.proxy_profile_id(),
            requests_per_minute: credential.requests_per_minute().map(RequestsPerMinute::get),
            enabled: credential.enabled(),
            secret_version: credential.secret_version(),
            credential_generation: credential.credential_generation(),
            config_version: credential.config_version(),
            models: credential
                .models()
                .iter()
                .map(|model| ProviderCredentialModelResponse {
                    upstream_model: model.upstream_model().as_str().to_owned(),
                    public_model: model.public_alias().map(|alias| alias.as_str().to_owned()),
                })
                .collect(),
            runtime: runtime.into(),
            usage: upstream_usage::for_id(
                RoutingCredentialId::provider_credential(credential.id()),
                usage,
            ),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ProviderCredentialTestResponse {
    config_revision: u64,
    provider_endpoint_config_version: u64,
    credential_config_version: u64,
    credential_generation: u64,
    secret_version: u64,
    proxy_config_version: u64,
    credential_id: CredentialId,
    provider_endpoint_id: ProviderEndpointId,
    proxy_id: ProxyProfileId,
    reachable: bool,
    accepted: bool,
    catalog_valid: bool,
    status_code: Option<u16>,
    latency_ms: u64,
    auth_error_cleared: bool,
    error_stage: Option<&'static str>,
    failure_scope: Option<&'static str>,
    models: Vec<String>,
}

impl From<ProviderCredentialTestResult> for ProviderCredentialTestResponse {
    fn from(result: ProviderCredentialTestResult) -> Self {
        let (
            reachable,
            accepted,
            catalog_valid,
            status_code,
            auth_error_cleared,
            error_stage,
            failure_scope,
            models,
        ) = match result.outcome {
            ProviderCredentialTestOutcome::Accepted {
                status_code,
                auth_error_cleared,
                models,
            } => (
                true,
                true,
                true,
                Some(status_code),
                auth_error_cleared,
                None,
                None,
                models,
            ),
            ProviderCredentialTestOutcome::InvalidCatalog { status_code } => (
                true,
                true,
                false,
                Some(status_code),
                false,
                None,
                None,
                Vec::new(),
            ),
            ProviderCredentialTestOutcome::Rejected { status_code } => (
                true,
                false,
                false,
                Some(status_code),
                false,
                None,
                None,
                Vec::new(),
            ),
            ProviderCredentialTestOutcome::Failed { stage, scope } => (
                false,
                false,
                false,
                None,
                false,
                Some(stage.as_str()),
                Some(scope.as_str()),
                Vec::new(),
            ),
        };
        Self {
            config_revision: result.config_revision.get(),
            provider_endpoint_config_version: result.provider_endpoint_config_version,
            credential_config_version: result.credential_config_version,
            credential_generation: result.credential_generation,
            secret_version: result.secret_version,
            proxy_config_version: result.proxy_config_version,
            credential_id: result.credential_id,
            provider_endpoint_id: result.provider_endpoint_id,
            proxy_id: result.proxy_id,
            reachable,
            accepted,
            catalog_valid,
            status_code,
            latency_ms: result.latency_ms,
            auth_error_cleared,
            error_stage,
            failure_scope,
            models,
        }
    }
}
