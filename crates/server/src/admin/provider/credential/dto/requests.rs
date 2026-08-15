use any2api_domain::{
    ConfigRevision, CredentialKind, ProviderCredentialDraft, ProviderCredentialModel,
    ProxyProfileId, RequestsPerMinute,
};
use any2api_runtime::api::ProviderApiKeySecret;
use serde::Deserialize;

use crate::admin::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialModelsRequest {
    expected_revision: u64,
    expected_config_version: u64,
    models: Vec<ProviderCredentialModelEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProviderCredentialModelEntry {
    upstream_model: String,
    #[serde(default)]
    public_model: Option<String>,
}

impl ProviderCredentialModelsRequest {
    pub(crate) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, u64, Vec<ProviderCredentialModel>), AdminApiError> {
        let models = self
            .models
            .into_iter()
            .map(|entry| ProviderCredentialModel::new(entry.upstream_model, entry.public_model))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))?;
        Ok((
            parse_revision(self.expected_revision)?,
            parse_version(
                self.expected_config_version,
                "expected_config_version is invalid",
            )?,
            models,
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
    requests_per_minute: Option<u32>,
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
        let revision = parse_revision(self.expected_revision)?;
        let draft = build_draft(
            self.label,
            self.credential_kind,
            self.proxy_profile_id,
            self.requests_per_minute,
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
    requests_per_minute: Option<u32>,
    enabled: bool,
}

impl ProviderCredentialUpdateRequest {
    pub(crate) fn into_domain(
        self,
    ) -> Result<(ConfigRevision, u64, ProviderCredentialDraft), AdminApiError> {
        let revision = parse_revision(self.expected_revision)?;
        let config_version = parse_version(
            self.expected_config_version,
            "expected_config_version is invalid",
        )?;
        let draft = build_draft(
            self.label,
            CredentialKind::ApiKey,
            self.proxy_profile_id,
            self.requests_per_minute,
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
    requests_per_minute: Option<u32>,
    enabled: bool,
) -> Result<ProviderCredentialDraft, AdminApiError> {
    let requests_per_minute = requests_per_minute
        .map(RequestsPerMinute::new)
        .transpose()
        .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))?;
    ProviderCredentialDraft::new(
        label,
        credential_kind,
        proxy_profile_id,
        requests_per_minute,
        enabled,
    )
    .map_err(|error| AdminApiError::invalid_provider_credential(error.to_string()))
}

fn parse_version(value: u64, message: &'static str) -> Result<u64, AdminApiError> {
    (value > 0)
        .then_some(value)
        .ok_or_else(|| AdminApiError::invalid_request(message))
}
