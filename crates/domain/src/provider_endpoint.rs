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
    upstream_protocol_dialect: Option<ProtocolDialect>,
    enabled: bool,
}

impl ProviderEndpointDraft {
    pub fn new(
        name: impl Into<String>,
        provider_kind: ProviderKind,
        base_url: impl Into<String>,
        protocol_dialect: ProtocolDialect,
        enabled: bool,
    ) -> Result<Self, ProviderEndpointValidationError> {
        Self::with_upstream_protocol(
            name,
            provider_kind,
            base_url,
            protocol_dialect,
            None,
            enabled,
        )
    }

    pub fn with_upstream_protocol(
        name: impl Into<String>,
        provider_kind: ProviderKind,
        base_url: impl Into<String>,
        protocol_dialect: ProtocolDialect,
        upstream_protocol_dialect: Option<ProtocolDialect>,
        enabled: bool,
    ) -> Result<Self, ProviderEndpointValidationError> {
        let upstream_protocol_dialect =
            upstream_protocol_dialect.filter(|upstream| *upstream != protocol_dialect);
        let base_url = ProviderBaseUrl::parse(base_url)?;

        Ok(Self {
            name: validate_name(name.into())?,
            provider_kind,
            base_url,
            protocol_dialect,
            upstream_protocol_dialect,
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
    pub const fn upstream_protocol_dialect(&self) -> Option<ProtocolDialect> {
        self.upstream_protocol_dialect
    }

    #[must_use]
    pub const fn effective_upstream_protocol_dialect(&self) -> ProtocolDialect {
        match self.upstream_protocol_dialect {
            Some(dialect) => dialect,
            None => self.protocol_dialect,
        }
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
    upstream_protocol_dialect: Option<ProtocolDialect>,
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
    pub const fn upstream_protocol_dialect(&self) -> Option<ProtocolDialect> {
        self.upstream_protocol_dialect
    }

    #[must_use]
    pub const fn effective_upstream_protocol_dialect(&self) -> ProtocolDialect {
        match self.upstream_protocol_dialect {
            Some(dialect) => dialect,
            None => self.protocol_dialect,
        }
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
            upstream_protocol_dialect: draft.upstream_protocol_dialect,
            enabled: draft.enabled,
            config_version,
        }
    }

    fn matches_draft(&self, draft: &ProviderEndpointDraft) -> bool {
        self.name == draft.name
            && self.provider_kind == draft.provider_kind
            && self.base_url == draft.base_url
            && self.protocol_dialect == draft.protocol_dialect
            && self.upstream_protocol_dialect == draft.upstream_protocol_dialect
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

#[cfg(test)]
mod tests {
    use super::ProviderEndpointDraft;
    use crate::{ProtocolDialect, ProviderKind};

    #[test]
    fn upstream_protocol_is_optional_and_same_protocol_normalizes_to_none() {
        let bridged = ProviderEndpointDraft::with_upstream_protocol(
            "Compatible",
            ProviderKind::Codex,
            "https://api.example.com",
            ProtocolDialect::OpenAiResponses,
            Some(ProtocolDialect::OpenAiChatCompletions),
            true,
        )
        .expect("Responses to Chat is a valid provider protocol pair");
        assert_eq!(
            bridged.effective_upstream_protocol_dialect(),
            ProtocolDialect::OpenAiChatCompletions
        );

        let direct = ProviderEndpointDraft::with_upstream_protocol(
            "Direct",
            ProviderKind::Codex,
            "https://api.example.com",
            ProtocolDialect::OpenAiResponses,
            Some(ProtocolDialect::OpenAiResponses),
            true,
        )
        .expect("same dialect normalizes to direct");
        assert_eq!(direct.upstream_protocol_dialect(), None);
    }
}
