use std::{collections::HashMap, time::Instant};

use any2api_domain::ProviderKind;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use super::error::OAuthError;

pub(super) const SESSION_TTL_SECONDS: u64 = 600;
const MAX_ACTIVE_SESSIONS: usize = 64;

pub(super) struct PreparedSession {
    pub(super) id: String,
    pub(super) state: String,
    pub(super) code_challenge: String,
    pub(super) session: OAuthSession,
}

pub(super) struct OAuthSession {
    pub(super) provider: ProviderKind,
    pub(super) redirect_uri: &'static str,
    state: SecretString,
    code_verifier: SecretString,
    expires_at: Instant,
}

impl OAuthSession {
    pub(super) fn prepare(
        provider: ProviderKind,
        redirect_uri: &'static str,
        now: Instant,
    ) -> Result<PreparedSession, OAuthError> {
        let id = random_urlsafe()?;
        let state = random_urlsafe()?;
        let code_verifier = random_urlsafe()?;
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        Ok(PreparedSession {
            id,
            state: state.clone(),
            code_challenge,
            session: Self {
                provider,
                redirect_uri,
                state: SecretString::from(state),
                code_verifier: SecretString::from(code_verifier),
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
pub(super) struct OAuthSessionStore {
    sessions: HashMap<String, OAuthSession>,
}

impl OAuthSessionStore {
    pub(super) fn insert(
        &mut self,
        id: String,
        session: OAuthSession,
        now: Instant,
    ) -> Result<(), OAuthError> {
        self.sessions.retain(|_, session| session.expires_at > now);
        if self.sessions.len() >= MAX_ACTIVE_SESSIONS {
            return Err(OAuthError::SessionCapacity);
        }
        self.sessions.insert(id, session);
        Ok(())
    }

    pub(super) fn take(&mut self, id: &str, now: Instant) -> Result<OAuthSession, OAuthError> {
        let session = self.sessions.remove(id).ok_or(OAuthError::InvalidSession)?;
        if session.expires_at <= now {
            return Err(OAuthError::SessionExpired);
        }
        Ok(session)
    }
}

fn random_urlsafe() -> Result<String, OAuthError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|_| OAuthError::RandomGeneration)?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}
