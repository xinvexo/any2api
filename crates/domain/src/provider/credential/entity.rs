use thiserror::Error;

use crate::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ModelNameValidationError,
    ProviderEndpointId, ProxyProfileId, RequestsPerMinute, UpstreamModelName,
};

use super::model::ProviderCredentialModel;

const MAX_CREDENTIAL_LABEL_CHARS: usize = 100;
const MAX_CREDENTIAL_VERSION: u64 = u32::MAX as u64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCredentialDraft {
    label: String,
    credential_kind: CredentialKind,
    proxy_profile_id: ProxyProfileId,
    requests_per_minute: Option<RequestsPerMinute>,
    enabled: bool,
}

impl ProviderCredentialDraft {
    pub fn new(
        label: impl Into<String>,
        credential_kind: CredentialKind,
        proxy_profile_id: ProxyProfileId,
        requests_per_minute: Option<RequestsPerMinute>,
        enabled: bool,
    ) -> Result<Self, ProviderCredentialValidationError> {
        Ok(Self {
            label: validate_label(label.into())?,
            credential_kind,
            proxy_profile_id,
            requests_per_minute,
            enabled,
        })
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn credential_kind(&self) -> CredentialKind {
        self.credential_kind
    }

    #[must_use]
    pub const fn proxy_profile_id(&self) -> ProxyProfileId {
        self.proxy_profile_id
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCredential {
    id: CredentialId,
    provider_endpoint_id: ProviderEndpointId,
    label: String,
    credential_kind: CredentialKind,
    fingerprint: CredentialSecretFingerprint,
    proxy_profile_id: ProxyProfileId,
    requests_per_minute: Option<RequestsPerMinute>,
    enabled: bool,
    secret_version: u64,
    credential_generation: u64,
    config_version: u64,
    models: Vec<ProviderCredentialModel>,
}

impl ProviderCredential {
    pub fn create(
        id: CredentialId,
        provider_endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        fingerprint: CredentialSecretFingerprint,
    ) -> Self {
        Self::from_parts(
            id,
            provider_endpoint_id,
            draft,
            fingerprint,
            1,
            1,
            1,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: CredentialId,
        provider_endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        fingerprint: CredentialSecretFingerprint,
        secret_version: u64,
        credential_generation: u64,
        config_version: u64,
        models: Vec<ProviderCredentialModel>,
    ) -> Result<Self, ProviderCredentialValidationError> {
        if !valid_version(secret_version)
            || !valid_version(credential_generation)
            || !valid_version(config_version)
        {
            return Err(ProviderCredentialValidationError::InvalidVersion);
        }
        Ok(Self::from_parts(
            id,
            provider_endpoint_id,
            draft,
            fingerprint,
            secret_version,
            credential_generation,
            config_version,
            validate_models(models)?,
        ))
    }

    pub fn updated(
        &self,
        draft: ProviderCredentialDraft,
    ) -> Result<Self, ProviderCredentialValidationError> {
        if self.matches_draft(&draft) {
            return Ok(self.clone());
        }
        let config_version = next_version(self.config_version)?;
        let credential_generation = if !self.enabled && draft.enabled() {
            next_version(self.credential_generation)?
        } else {
            self.credential_generation
        };
        Ok(Self::from_parts(
            self.id,
            self.provider_endpoint_id,
            draft,
            self.fingerprint.clone(),
            self.secret_version,
            credential_generation,
            config_version,
            self.models.clone(),
        ))
    }

    pub fn with_models(
        &self,
        models: Vec<ProviderCredentialModel>,
    ) -> Result<Self, ProviderCredentialValidationError> {
        let models = validate_models(models)?;
        if self.models == models {
            return Ok(self.clone());
        }
        Ok(Self {
            models,
            config_version: next_version(self.config_version)?,
            ..self.clone()
        })
    }

    pub fn rotated(
        &self,
        fingerprint: CredentialSecretFingerprint,
    ) -> Result<Self, ProviderCredentialValidationError> {
        Ok(Self {
            fingerprint,
            secret_version: next_version(self.secret_version)?,
            credential_generation: next_version(self.credential_generation)?,
            config_version: next_version(self.config_version)?,
            models: Vec::new(),
            ..self.clone()
        })
    }

    pub fn next_generation(&self) -> Result<Self, ProviderCredentialValidationError> {
        Ok(Self {
            credential_generation: next_version(self.credential_generation)?,
            ..self.clone()
        })
    }

    #[must_use]
    pub const fn id(&self) -> CredentialId {
        self.id
    }

    #[must_use]
    pub const fn provider_endpoint_id(&self) -> ProviderEndpointId {
        self.provider_endpoint_id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn label_key(&self) -> String {
        self.label.to_lowercase()
    }

    #[must_use]
    pub const fn credential_kind(&self) -> CredentialKind {
        self.credential_kind
    }

    #[must_use]
    pub const fn fingerprint(&self) -> &CredentialSecretFingerprint {
        &self.fingerprint
    }

    #[must_use]
    pub const fn proxy_profile_id(&self) -> ProxyProfileId {
        self.proxy_profile_id
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn secret_version(&self) -> u64 {
        self.secret_version
    }

    #[must_use]
    pub const fn credential_generation(&self) -> u64 {
        self.credential_generation
    }

    #[must_use]
    pub const fn config_version(&self) -> u64 {
        self.config_version
    }

    #[must_use]
    pub fn models(&self) -> &[ProviderCredentialModel] {
        &self.models
    }

    #[must_use]
    pub fn supports_model(&self, model: &UpstreamModelName) -> bool {
        self.models
            .binary_search_by(|entry| entry.upstream_model().cmp(model))
            .is_ok()
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        id: CredentialId,
        provider_endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        fingerprint: CredentialSecretFingerprint,
        secret_version: u64,
        credential_generation: u64,
        config_version: u64,
        models: Vec<ProviderCredentialModel>,
    ) -> Self {
        Self {
            id,
            provider_endpoint_id,
            label: draft.label,
            credential_kind: draft.credential_kind,
            fingerprint,
            proxy_profile_id: draft.proxy_profile_id,
            requests_per_minute: draft.requests_per_minute,
            enabled: draft.enabled,
            secret_version,
            credential_generation,
            config_version,
            models,
        }
    }

    fn matches_draft(&self, draft: &ProviderCredentialDraft) -> bool {
        self.label == draft.label
            && self.credential_kind == draft.credential_kind
            && self.proxy_profile_id == draft.proxy_profile_id
            && self.requests_per_minute == draft.requests_per_minute
            && self.enabled == draft.enabled
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProviderCredentialValidationError {
    #[error("credential label must not be empty")]
    EmptyLabel,
    #[error("credential label must be trimmed")]
    LabelNotTrimmed,
    #[error("credential label is too long")]
    LabelTooLong,
    #[error("credential version is invalid")]
    InvalidVersion,
    #[error("credential id is duplicated")]
    DuplicateId,
    #[error("credential label is duplicated for this endpoint")]
    DuplicateLabel,
    #[error("credential model name is invalid: {0}")]
    InvalidModel(ModelNameValidationError),
    #[error("credential public model name is invalid: {0}")]
    InvalidPublicModel(ModelNameValidationError),
    #[error("credential model is duplicated")]
    DuplicateModel,
    #[error("credential public model name is duplicated")]
    DuplicatePublicModel,
    #[error("credential references a missing provider endpoint")]
    MissingProviderEndpoint,
    #[error("credential references a missing proxy profile")]
    MissingProxyProfile,
}

fn validate_label(label: String) -> Result<String, ProviderCredentialValidationError> {
    if label.trim().is_empty() {
        return Err(ProviderCredentialValidationError::EmptyLabel);
    }
    if label.trim() != label {
        return Err(ProviderCredentialValidationError::LabelNotTrimmed);
    }
    if label.chars().count() > MAX_CREDENTIAL_LABEL_CHARS {
        return Err(ProviderCredentialValidationError::LabelTooLong);
    }
    Ok(label)
}

fn validate_models(
    mut models: Vec<ProviderCredentialModel>,
) -> Result<Vec<ProviderCredentialModel>, ProviderCredentialValidationError> {
    models.sort();
    if models
        .windows(2)
        .any(|pair| pair[0].upstream_model() == pair[1].upstream_model())
    {
        return Err(ProviderCredentialValidationError::DuplicateModel);
    }
    let mut public_names = models
        .iter()
        .map(ProviderCredentialModel::public_model)
        .collect::<Vec<_>>();
    public_names.sort();
    if public_names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProviderCredentialValidationError::DuplicatePublicModel);
    }
    Ok(models)
}

const fn valid_version(value: u64) -> bool {
    value > 0 && value <= MAX_CREDENTIAL_VERSION
}

fn next_version(value: u64) -> Result<u64, ProviderCredentialValidationError> {
    value
        .checked_add(1)
        .filter(|next| *next <= MAX_CREDENTIAL_VERSION)
        .ok_or(ProviderCredentialValidationError::InvalidVersion)
}
