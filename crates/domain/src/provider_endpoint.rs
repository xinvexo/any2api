use thiserror::Error;

use crate::{
    ProtocolDialect, ProviderBaseUrl, ProviderEndpointId, ProviderKind,
    provider_base_url::ProviderUrlValidationError,
};

const MAX_ENDPOINT_NAME_CHARS: usize = 100;
const MAX_ENDPOINT_CONFIG_VERSION: u64 = u32::MAX as u64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEndpointDraft {
    name: String,
    provider_kind: ProviderKind,
    base_url: ProviderBaseUrl,
    protocol_dialect: ProtocolDialect,
    allow_insecure_http: bool,
    allow_private_network: bool,
    enabled: bool,
}

impl ProviderEndpointDraft {
    pub fn new(
        name: impl Into<String>,
        provider_kind: ProviderKind,
        base_url: impl Into<String>,
        protocol_dialect: ProtocolDialect,
        allow_insecure_http: bool,
        allow_private_network: bool,
        enabled: bool,
    ) -> Result<Self, ProviderEndpointValidationError> {
        validate_dialect(provider_kind, protocol_dialect)?;
        let base_url =
            ProviderBaseUrl::parse(base_url, allow_insecure_http, allow_private_network)?;

        Ok(Self {
            name: validate_name(name.into())?,
            provider_kind,
            base_url,
            protocol_dialect,
            allow_insecure_http,
            allow_private_network,
            enabled,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub fn base_url(&self) -> &ProviderBaseUrl {
        &self.base_url
    }

    #[must_use]
    pub const fn protocol_dialect(&self) -> ProtocolDialect {
        self.protocol_dialect
    }

    #[must_use]
    pub const fn allow_insecure_http(&self) -> bool {
        self.allow_insecure_http
    }

    #[must_use]
    pub const fn allow_private_network(&self) -> bool {
        self.allow_private_network
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderEndpoint {
    id: ProviderEndpointId,
    name: String,
    provider_kind: ProviderKind,
    base_url: ProviderBaseUrl,
    protocol_dialect: ProtocolDialect,
    allow_insecure_http: bool,
    allow_private_network: bool,
    enabled: bool,
    config_version: u64,
}

impl ProviderEndpoint {
    pub fn create(
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
    ) -> Result<Self, ProviderEndpointValidationError> {
        Ok(Self::from_draft(id, draft, 1))
    }

    pub fn restore(
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
        config_version: u64,
    ) -> Result<Self, ProviderEndpointValidationError> {
        if config_version == 0 || config_version > MAX_ENDPOINT_CONFIG_VERSION {
            return Err(ProviderEndpointValidationError::InvalidConfigVersion);
        }
        Ok(Self::from_draft(id, draft, config_version))
    }

    pub fn updated(
        &self,
        draft: ProviderEndpointDraft,
    ) -> Result<Self, ProviderEndpointValidationError> {
        if self.matches_draft(&draft) {
            return Ok(self.clone());
        }
        let version = self
            .config_version
            .checked_add(1)
            .filter(|value| *value <= MAX_ENDPOINT_CONFIG_VERSION)
            .ok_or(ProviderEndpointValidationError::InvalidConfigVersion)?;
        Ok(Self::from_draft(self.id, draft, version))
    }

    #[must_use]
    pub const fn id(&self) -> ProviderEndpointId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub fn base_url(&self) -> &ProviderBaseUrl {
        &self.base_url
    }

    #[must_use]
    pub const fn protocol_dialect(&self) -> ProtocolDialect {
        self.protocol_dialect
    }

    #[must_use]
    pub const fn allow_insecure_http(&self) -> bool {
        self.allow_insecure_http
    }

    #[must_use]
    pub const fn allow_private_network(&self) -> bool {
        self.allow_private_network
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn config_version(&self) -> u64 {
        self.config_version
    }

    #[must_use]
    pub fn name_key(&self) -> String {
        self.name.to_lowercase()
    }

    fn from_draft(
        id: ProviderEndpointId,
        draft: ProviderEndpointDraft,
        config_version: u64,
    ) -> Self {
        Self {
            id,
            name: draft.name,
            provider_kind: draft.provider_kind,
            base_url: draft.base_url,
            protocol_dialect: draft.protocol_dialect,
            allow_insecure_http: draft.allow_insecure_http,
            allow_private_network: draft.allow_private_network,
            enabled: draft.enabled,
            config_version,
        }
    }

    fn matches_draft(&self, draft: &ProviderEndpointDraft) -> bool {
        self.name == draft.name
            && self.provider_kind == draft.provider_kind
            && self.base_url == draft.base_url
            && self.protocol_dialect == draft.protocol_dialect
            && self.allow_insecure_http == draft.allow_insecure_http
            && self.allow_private_network == draft.allow_private_network
            && self.enabled == draft.enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum ProviderEndpointValidationError {
    #[error("provider endpoint name must not be empty")]
    EmptyName,
    #[error("provider endpoint name must be trimmed")]
    NameNotTrimmed,
    #[error("provider endpoint name is too long")]
    NameTooLong,
    #[error("provider URL is invalid: {0}")]
    InvalidBaseUrl(#[from] ProviderUrlValidationError),
    #[error("provider and protocol dialect are incompatible")]
    IncompatibleDialect,
    #[error("provider endpoint configuration version is invalid")]
    InvalidConfigVersion,
    #[error("provider endpoint id is duplicated")]
    DuplicateId,
    #[error("provider endpoint name is duplicated")]
    DuplicateName,
}

fn validate_name(name: String) -> Result<String, ProviderEndpointValidationError> {
    if name.trim().is_empty() {
        return Err(ProviderEndpointValidationError::EmptyName);
    }
    if name.trim() != name {
        return Err(ProviderEndpointValidationError::NameNotTrimmed);
    }
    if name.chars().count() > MAX_ENDPOINT_NAME_CHARS {
        return Err(ProviderEndpointValidationError::NameTooLong);
    }
    Ok(name)
}

fn validate_dialect(
    provider_kind: ProviderKind,
    protocol_dialect: ProtocolDialect,
) -> Result<(), ProviderEndpointValidationError> {
    let compatible = matches!(
        (provider_kind, protocol_dialect),
        (ProviderKind::Codex, ProtocolDialect::OpenAiResponses)
            | (ProviderKind::Claude, ProtocolDialect::AnthropicMessages)
    );
    compatible
        .then_some(())
        .ok_or(ProviderEndpointValidationError::IncompatibleDialect)
}

#[cfg(test)]
mod tests {
    use super::{ProviderEndpointDraft, ProviderEndpointValidationError};
    use crate::{ProtocolDialect, ProviderKind};

    #[test]
    fn only_first_release_provider_dialects_are_accepted() {
        assert!(
            ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                false,
                false,
                true
            )
            .is_ok()
        );
        assert_eq!(
            ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::AnthropicMessages,
                false,
                false,
                true
            )
            .expect_err("cross protocol endpoint must fail"),
            ProviderEndpointValidationError::IncompatibleDialect
        );
    }
}
