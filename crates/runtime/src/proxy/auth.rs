use std::{collections::HashMap, fmt};

use any2api_domain::{ProxyConfiguration, ProxyProfile, ProxyProfileId};
use any2api_storage::api::{StoredProxyPassword, StoredProxyPasswords};
use any2api_transport::api::ProxyCredentials;
use secrecy::ExposeSecret;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum ProxyAuthMaterialError {
    #[error("proxy password references unknown proxy {0}")]
    UnknownProxy(ProxyProfileId),
    #[error("proxy password exists without authentication metadata for proxy {0}")]
    AuthenticationMissing(ProxyProfileId),
    #[error("proxy password version does not match proxy {0}")]
    VersionMismatch(ProxyProfileId),
    #[error("proxy password is not valid UTF-8 for proxy {0}")]
    InvalidUtf8(ProxyProfileId),
    #[error("duplicate proxy password material for proxy {0}")]
    Duplicate(ProxyProfileId),
    #[error("proxy password material is missing for authenticated proxy {0}")]
    PasswordMissing(ProxyProfileId),
}

fn compile_material(
    configuration: &ProxyConfiguration,
    stored: StoredProxyPassword,
) -> Result<(ProxyProfileId, ProxyCredentials), ProxyAuthMaterialError> {
    let id = stored.proxy_profile_id();
    let profile = configuration
        .get(id)
        .ok_or(ProxyAuthMaterialError::UnknownProxy(id))?;
    let authentication = profile
        .authentication()
        .ok_or(ProxyAuthMaterialError::AuthenticationMissing(id))?;
    if stored.authentication_version() != profile.authentication_version() {
        return Err(ProxyAuthMaterialError::VersionMismatch(id));
    }
    let secret = stored.into_secret();
    let password = String::from_utf8(secret.expose_secret().to_vec())
        .map_err(|_| ProxyAuthMaterialError::InvalidUtf8(id))?;
    Ok((
        id,
        ProxyCredentials::new(authentication.username().to_owned(), password),
    ))
}

pub(crate) struct ProxyAuthMaterials {
    by_id: HashMap<ProxyProfileId, ProxyCredentials>,
}

impl ProxyAuthMaterials {
    pub(crate) fn compile(
        configuration: &ProxyConfiguration,
        stored: StoredProxyPasswords,
    ) -> Result<Self, ProxyAuthMaterialError> {
        let mut by_id = HashMap::new();
        for stored in stored.into_entries() {
            let (id, credentials) = compile_material(configuration, stored)?;
            if by_id.insert(id, credentials).is_some() {
                return Err(ProxyAuthMaterialError::Duplicate(id));
            }
        }
        if let Some(profile) = configuration.profiles().iter().find(|profile| {
            profile.authentication().is_some() && !by_id.contains_key(&profile.id())
        }) {
            return Err(ProxyAuthMaterialError::PasswordMissing(profile.id()));
        }
        Ok(Self { by_id })
    }

    pub(crate) fn credentials_for(&self, profile: &ProxyProfile) -> Option<&ProxyCredentials> {
        profile.authentication()?;
        self.by_id.get(&profile.id())
    }
}

impl fmt::Debug for ProxyAuthMaterials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProxyAuthMaterials")
            .field("entry_count", &self.by_id.len())
            .finish()
    }
}
