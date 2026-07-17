use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ProxyProfileId, proxy_address::ProxyAddress};

const MAX_PROXY_NAME_CHARS: usize = 100;
const MAX_PROXY_CONFIG_VERSION: u64 = u32::MAX as u64;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyKind {
    Direct,
    Http,
    Socks5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyDraft {
    name: String,
    kind: ProxyKind,
    address: ProxyAddress,
    enabled: bool,
}

impl ProxyDraft {
    pub fn new(
        name: impl Into<String>,
        kind: ProxyKind,
        address: ProxyAddress,
        enabled: bool,
    ) -> Result<Self, ProxyValidationError> {
        if kind == ProxyKind::Direct {
            return Err(ProxyValidationError::DirectCannotBeCreated);
        }

        Ok(Self {
            name: validate_name(name.into())?,
            kind,
            address,
            enabled,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> ProxyKind {
        self.kind
    }

    #[must_use]
    pub const fn address(&self) -> &ProxyAddress {
        &self.address
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyProfile {
    id: ProxyProfileId,
    name: String,
    kind: ProxyKind,
    address: Option<ProxyAddress>,
    enabled: bool,
    config_version: u64,
}

impl ProxyProfile {
    #[must_use]
    pub fn direct() -> Self {
        Self {
            id: ProxyProfileId::DIRECT,
            name: "DIRECT".to_owned(),
            kind: ProxyKind::Direct,
            address: None,
            enabled: true,
            config_version: 1,
        }
    }

    pub fn create(id: ProxyProfileId, draft: ProxyDraft) -> Result<Self, ProxyValidationError> {
        if id == ProxyProfileId::DIRECT {
            return Err(ProxyValidationError::DirectIdReserved);
        }

        Ok(Self::from_draft(id, draft, 1))
    }

    pub fn restore(
        id: ProxyProfileId,
        name: impl Into<String>,
        kind: ProxyKind,
        address: Option<ProxyAddress>,
        enabled: bool,
        config_version: u64,
    ) -> Result<Self, ProxyValidationError> {
        if config_version == 0 || config_version > MAX_PROXY_CONFIG_VERSION {
            return Err(ProxyValidationError::InvalidConfigVersion);
        }
        if id == ProxyProfileId::DIRECT {
            let direct = Self::direct();
            if name.into() != direct.name || kind != direct.kind || address.is_some() || !enabled {
                return Err(ProxyValidationError::DirectInvariant);
            }
            return Ok(Self {
                config_version,
                ..direct
            });
        }
        if kind == ProxyKind::Direct {
            return Err(ProxyValidationError::DirectIdReserved);
        }
        let address = address.ok_or(ProxyValidationError::MissingAddress)?;
        let draft = ProxyDraft::new(name, kind, address, enabled)?;

        Ok(Self::from_draft(id, draft, config_version))
    }

    pub fn updated(&self, draft: ProxyDraft) -> Result<Self, ProxyValidationError> {
        if self.is_built_in() {
            return Err(ProxyValidationError::DirectInvariant);
        }
        if self.matches_draft(&draft) {
            return Ok(self.clone());
        }
        let version = self
            .config_version
            .checked_add(1)
            .ok_or(ProxyValidationError::InvalidConfigVersion)?;
        if version > MAX_PROXY_CONFIG_VERSION {
            return Err(ProxyValidationError::InvalidConfigVersion);
        }

        Ok(Self::from_draft(self.id, draft, version))
    }

    #[must_use]
    pub const fn id(&self) -> ProxyProfileId {
        self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> ProxyKind {
        self.kind
    }

    #[must_use]
    pub const fn address(&self) -> Option<&ProxyAddress> {
        self.address.as_ref()
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
    pub fn is_built_in(&self) -> bool {
        self.id == ProxyProfileId::DIRECT
    }

    #[must_use]
    pub fn name_key(&self) -> String {
        self.name.to_lowercase()
    }

    fn from_draft(id: ProxyProfileId, draft: ProxyDraft, config_version: u64) -> Self {
        Self {
            id,
            name: draft.name,
            kind: draft.kind,
            address: Some(draft.address),
            enabled: draft.enabled,
            config_version,
        }
    }

    fn matches_draft(&self, draft: &ProxyDraft) -> bool {
        self.name == draft.name
            && self.kind == draft.kind
            && self.address.as_ref() == Some(&draft.address)
            && self.enabled == draft.enabled
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProxyValidationError {
    #[error("proxy name must not be empty")]
    EmptyName,
    #[error("proxy name must be trimmed")]
    NameNotTrimmed,
    #[error("proxy name is too long")]
    NameTooLong,
    #[error("proxy host is invalid")]
    InvalidHost,
    #[error("proxy port is invalid")]
    InvalidPort,
    #[error("DIRECT cannot be created by the administrator")]
    DirectCannotBeCreated,
    #[error("the DIRECT proxy id is reserved")]
    DirectIdReserved,
    #[error("the built-in DIRECT proxy is invalid")]
    DirectInvariant,
    #[error("a network proxy requires an address")]
    MissingAddress,
    #[error("proxy configuration version is invalid")]
    InvalidConfigVersion,
    #[error("proxy id is duplicated")]
    DuplicateId,
    #[error("proxy name is duplicated")]
    DuplicateName,
    #[error("the built-in DIRECT proxy is missing")]
    MissingDirect,
    #[error("global proxy does not exist")]
    GlobalProxyMissing,
    #[error("global proxy is disabled")]
    GlobalProxyDisabled,
}

fn validate_name(name: String) -> Result<String, ProxyValidationError> {
    if name.trim().is_empty() {
        return Err(ProxyValidationError::EmptyName);
    }
    if name.trim() != name {
        return Err(ProxyValidationError::NameNotTrimmed);
    }
    if name.chars().count() > MAX_PROXY_NAME_CHARS {
        return Err(ProxyValidationError::NameTooLong);
    }

    Ok(name)
}

#[cfg(test)]
mod tests {
    use crate::{ProxyAddress, ProxyKind, ProxyProfile, ProxyProfileId, ProxyValidationError};

    #[test]
    fn restored_config_version_respects_the_domain_limit() {
        let address = ProxyAddress::new("proxy.example.com", 8080).expect("address");
        let error = ProxyProfile::restore(
            ProxyProfileId::new(),
            "Proxy",
            ProxyKind::Http,
            Some(address),
            true,
            u32::MAX as u64 + 1,
        )
        .expect_err("oversized version must fail");

        assert_eq!(error, ProxyValidationError::InvalidConfigVersion);
    }
}
