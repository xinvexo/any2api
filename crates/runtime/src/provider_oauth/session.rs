use std::{collections::HashMap, time::Instant};

use any2api_domain::{ProviderCredentialDraft, ProviderEndpointId, ProviderKind, ProxyProfileId};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use super::error::ProviderOAuthError;

pub(crate) const SESSION_TTL_SECONDS: u64 = 600;
const MAX_ACTIVE_SESSIONS: usize = 64;

pub(crate) struct PreparedSession {
    pub(crate) id: String,
    pub(crate) state: String,
    pub(crate) code_challenge: String,
    pub(crate) session: OAuthSession,
}

pub(crate) struct OAuthSession {
    pub(super) provider_kind: ProviderKind,
    pub(super) endpoint_id: ProviderEndpointId,
    pub(super) endpoint_config_version: u64,
    pub(super) resolved_proxy_id: ProxyProfileId,
    pub(super) resolved_proxy_config_version: u64,
    pub(super) draft: ProviderCredentialDraft,
    state: SecretString,
    code_verifier: SecretString,
    pub(super) redirect_uri: &'static str,
    expires_at: Instant,
}

impl OAuthSession {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(
        provider_kind: ProviderKind,
        endpoint_id: ProviderEndpointId,
        endpoint_config_version: u64,
        resolved_proxy_id: ProxyProfileId,
        resolved_proxy_config_version: u64,
        draft: ProviderCredentialDraft,
        redirect_uri: &'static str,
        now: Instant,
    ) -> Result<PreparedSession, ProviderOAuthError> {
        let id = random_urlsafe()?;
        let state = random_urlsafe()?;
        let code_verifier = random_urlsafe()?;
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        Ok(PreparedSession {
            id,
            state: state.clone(),
            code_challenge,
            session: Self {
                provider_kind,
                endpoint_id,
                endpoint_config_version,
                resolved_proxy_id,
                resolved_proxy_config_version,
                draft,
                state: SecretString::from(state),
                code_verifier: SecretString::from(code_verifier),
                redirect_uri,
                expires_at: now + std::time::Duration::from_secs(SESSION_TTL_SECONDS),
            },
        })
    }

    pub(super) fn state(&self) -> &str {
        self.state.expose_secret()
    }

    pub(super) fn code_verifier(&self) -> &str {
        self.code_verifier.expose_secret()
    }
}

#[derive(Default)]
pub(crate) struct OAuthSessionStore {
    sessions: HashMap<String, OAuthSession>,
}

impl OAuthSessionStore {
    pub(crate) fn insert(
        &mut self,
        id: String,
        session: OAuthSession,
        now: Instant,
    ) -> Result<(), ProviderOAuthError> {
        self.sessions.retain(|_, session| session.expires_at > now);
        if self.sessions.len() >= MAX_ACTIVE_SESSIONS {
            return Err(ProviderOAuthError::SessionCapacity);
        }
        match self.sessions.entry(id) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(session);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                Err(ProviderOAuthError::RandomGeneration)
            }
        }
    }

    pub(crate) fn take(
        &mut self,
        id: &str,
        now: Instant,
    ) -> Result<OAuthSession, ProviderOAuthError> {
        let session = self
            .sessions
            .remove(id)
            .ok_or(ProviderOAuthError::InvalidSession)?;
        if session.expires_at <= now {
            return Err(ProviderOAuthError::SessionExpired);
        }
        Ok(session)
    }
}

fn random_urlsafe() -> Result<String, ProviderOAuthError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|_| ProviderOAuthError::RandomGeneration)?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}
