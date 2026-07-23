use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProviderCredential,
    ProviderCredentialDraft, ProviderEndpointId, ProxyProfileId,
};
use any2api_runtime::api::{
    ProviderApiKeySecret, ProviderCredentialTestOutcome, ProviderCredentialTestResult,
    PublishedSnapshot,
};
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

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
    ) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            provider_endpoint_id: endpoint_id,
            items: snapshot
                .provider_credentials()
                .for_endpoint(endpoint_id)
                .map(ProviderCredentialResponse::from)
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
    max_concurrency: u32,
    enabled: bool,
    secret_schema_version: u32,
    secret_version: u64,
    credential_generation: u64,
    config_version: u64,
    models: Vec<String>,
}

impl From<&ProviderCredential> for ProviderCredentialResponse {
    fn from(credential: &ProviderCredential) -> Self {
        Self {
            id: credential.id(),
            provider_endpoint_id: credential.provider_endpoint_id(),
            label: credential.label().to_owned(),
            credential_kind: credential.credential_kind(),
            fingerprint: credential.fingerprint().display(),
            secret_tail: credential.fingerprint().tail().map(str::to_owned),
            proxy_profile_id: credential.proxy_profile_id(),
            max_concurrency: credential.max_concurrency().get(),
            enabled: credential.enabled(),
            secret_schema_version: credential.secret_schema_version(),
            secret_version: credential.secret_version(),
            credential_generation: credential.credential_generation(),
            config_version: credential.config_version(),
            models: credential
                .models()
                .iter()
                .map(|model| model.as_str().to_owned())
                .collect(),
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialModelsRequest {
    expected_revision: u64,
    expected_config_version: u64,
    models: Vec<String>,
}

impl ProviderCredentialModelsRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, u64, Vec<String>), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
            self.models,
        ))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialCreateRequest {
    expected_revision: u64,
    label: String,
    credential_kind: CredentialKind,
    api_key: String,
    proxy_profile_id: ProxyProfileId,
    max_concurrency: u32,
    enabled: bool,
}

impl ProviderCredentialCreateRequest {
    pub(crate) fn into_domain(
        self,
    ) -> Result<
        (
            ConfigRevision,
            ProviderCredentialDraft,
            ProviderApiKeySecret,
        ),
        AdminApiError,
    > {
        if self.credential_kind != CredentialKind::ApiKey {
            return Err(AdminApiError::invalid_provider_credential(
                "OAuth2 credentials must be created through the OAuth login flow",
            ));
        }
        let revision = parse_revision(self.expected_revision)?;
        let draft = build_draft(
            self.label,
            self.credential_kind,
            self.proxy_profile_id,
            self.max_concurrency,
            self.enabled,
        )?;
        Ok((revision, draft, ProviderApiKeySecret::new(self.api_key)))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialUpdateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    label: String,
    proxy_profile_id: ProxyProfileId,
    max_concurrency: u32,
    enabled: bool,
}

impl ProviderCredentialUpdateRequest {
    pub(crate) fn into_domain(
        self,
        credential_kind: CredentialKind,
    ) -> Result<(ConfigRevision, u64, ProviderCredentialDraft), AdminApiError> {
        let revision = parse_revision(self.expected_revision)?;
        let config_version = parse_version(
            self.expected_config_version,
            "expected_config_version is invalid",
        )?;
        let draft = build_draft(
            self.label,
            credential_kind,
            self.proxy_profile_id,
            self.max_concurrency,
            self.enabled,
        )?;
        Ok((revision, config_version, draft))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialRotateRequest {
    expected_revision: u64,
    expected_config_version: u64,
    expected_secret_version: u64,
    api_key: String,
}

impl ProviderCredentialRotateRequest {
    pub(crate) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, u64, u64, ProviderApiKeySecret), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
            parse_version(
                self.expected_secret_version,
                "expected_secret_version is invalid",
            )?,
            ProviderApiKeySecret::new(self.api_key),
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialDeleteQuery {
    expected_revision: u64,
    expected_config_version: u64,
}

impl ProviderCredentialDeleteQuery {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, u64), AdminApiError> {
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
        ))
    }
}

fn build_draft(
    label: String,
    credential_kind: CredentialKind,
    proxy_profile_id: ProxyProfileId,
    max_concurrency: u32,
    enabled: bool,
) -> Result<ProviderCredentialDraft, AdminApiError> {
    let max_concurrency = MaxConcurrency::new(max_concurrency)
        .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))?;
    ProviderCredentialDraft::new(
        label,
        credential_kind,
        proxy_profile_id,
        max_concurrency,
        enabled,
    )
    .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))
}

fn parse_version(value: u64, message: &'static str) -> Result<u64, AdminApiError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AdminApiError::invalid_request(message))
}
