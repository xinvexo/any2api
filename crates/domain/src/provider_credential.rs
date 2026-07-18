use thiserror::Error;

use crate::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProviderEndpointId,
    ProxyProfileId,
};

const MAX_CREDENTIAL_LABEL_CHARS: usize = 100;
const MAX_CREDENTIAL_VERSION: u64 = u32::MAX as u64;
pub const API_KEY_SECRET_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCredentialDraft {
    label: String,
    credential_kind: CredentialKind,
    proxy_profile_id: ProxyProfileId,
    max_concurrency: MaxConcurrency,
    enabled: bool,
}

impl ProviderCredentialDraft {
    pub fn new(
        label: impl Into<String>,
        credential_kind: CredentialKind,
        proxy_profile_id: ProxyProfileId,
        max_concurrency: MaxConcurrency,
        enabled: bool,
    ) -> Result<Self, ProviderCredentialValidationError> {
        Ok(Self {
            label: validate_label(label.into())?,
            credential_kind,
            proxy_profile_id,
            max_concurrency,
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
    pub const fn max_concurrency(&self) -> MaxConcurrency {
        self.max_concurrency
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
    max_concurrency: MaxConcurrency,
    enabled: bool,
    secret_schema_version: u32,
    secret_version: u64,
    credential_generation: u64,
    config_version: u64,
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
            API_KEY_SECRET_SCHEMA_VERSION,
            1,
            1,
            1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: CredentialId,
        provider_endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        fingerprint: CredentialSecretFingerprint,
        secret_schema_version: u32,
        secret_version: u64,
        credential_generation: u64,
        config_version: u64,
    ) -> Result<Self, ProviderCredentialValidationError> {
        if secret_schema_version != API_KEY_SECRET_SCHEMA_VERSION
            || !valid_version(secret_version)
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
            secret_schema_version,
            secret_version,
            credential_generation,
            config_version,
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
            self.secret_schema_version,
            self.secret_version,
            credential_generation,
            config_version,
        ))
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
    pub const fn max_concurrency(&self) -> MaxConcurrency {
        self.max_concurrency
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn secret_schema_version(&self) -> u32 {
        self.secret_schema_version
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

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        id: CredentialId,
        provider_endpoint_id: ProviderEndpointId,
        draft: ProviderCredentialDraft,
        fingerprint: CredentialSecretFingerprint,
        secret_schema_version: u32,
        secret_version: u64,
        credential_generation: u64,
        config_version: u64,
    ) -> Self {
        Self {
            id,
            provider_endpoint_id,
            label: draft.label,
            credential_kind: draft.credential_kind,
            fingerprint,
            proxy_profile_id: draft.proxy_profile_id,
            max_concurrency: draft.max_concurrency,
            enabled: draft.enabled,
            secret_schema_version,
            secret_version,
            credential_generation,
            config_version,
        }
    }

    fn matches_draft(&self, draft: &ProviderCredentialDraft) -> bool {
        self.label == draft.label
            && self.credential_kind == draft.credential_kind
            && self.proxy_profile_id == draft.proxy_profile_id
            && self.max_concurrency == draft.max_concurrency
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

const fn valid_version(value: u64) -> bool {
    value > 0 && value <= MAX_CREDENTIAL_VERSION
}

fn next_version(value: u64) -> Result<u64, ProviderCredentialValidationError> {
    value
        .checked_add(1)
        .filter(|next| *next <= MAX_CREDENTIAL_VERSION)
        .ok_or(ProviderCredentialValidationError::InvalidVersion)
}
