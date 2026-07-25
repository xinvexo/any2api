use std::{collections::HashMap, time::Instant};

use any2api_domain::ProviderKind;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use super::error::OAuthError;

pub(super) const CALLBACK_SESSION_TTL_SECONDS: u64 = 600;
pub(super) const MAX_DEVICE_SESSION_TTL_SECONDS: u64 = 30 * 60;
const DEVICE_SLOW_DOWN_SECONDS: u64 = 5;
const MAX_ACTIVE_SESSIONS: usize = 64;

pub(super) struct PreparedAuthorizationCodeSession {
    pub(super) id: String,
    pub(super) state: String,
    pub(super) code_challenge: String,
    pub(super) session: AuthorizationCodeSession,
}

pub(super) struct AuthorizationCodeSession {
    pub(super) provider: ProviderKind,
    pub(super) redirect_uri: &'static str,
    state: SecretString,
    code_verifier: SecretString,
    expires_at: Instant,
}

impl AuthorizationCodeSession {
    pub(super) fn prepare(
        provider: ProviderKind,
        redirect_uri: &'static str,
        now: Instant,
    ) -> Result<PreparedAuthorizationCodeSession, OAuthError> {
        let id = random_urlsafe()?;
        let state = random_urlsafe()?;
        let code_verifier = random_urlsafe()?;
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        Ok(PreparedAuthorizationCodeSession {
            id,
            state: state.clone(),
            code_challenge,
            session: Self {
                provider,
                redirect_uri,
                state: SecretString::from(state),
                code_verifier: SecretString::from(code_verifier),
                expires_at: now + std::time::Duration::from_secs(CALLBACK_SESSION_TTL_SECONDS),
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

pub(super) struct PreparedDeviceCodeSession {
    pub(super) id: String,
    pub(super) expires_in_seconds: u64,
    pub(super) poll_interval_seconds: u64,
    pub(super) session: DeviceCodeSession,
}

pub(super) struct DeviceCodeSession {
    pub(super) provider: ProviderKind,
    device_code: SecretString,
    poll_interval_seconds: u64,
    next_poll_at: Instant,
    expires_at: Instant,
}

impl DeviceCodeSession {
    pub(super) fn prepare(
        provider: ProviderKind,
        device_code: &str,
        expires_in_seconds: u64,
        poll_interval_seconds: u64,
        now: Instant,
    ) -> Result<PreparedDeviceCodeSession, OAuthError> {
        if device_code.trim().is_empty() || expires_in_seconds == 0 || poll_interval_seconds == 0 {
            return Err(OAuthError::TokenResponseInvalid);
        }
        let expires_in_seconds = expires_in_seconds.min(MAX_DEVICE_SESSION_TTL_SECONDS);
        let poll_interval_seconds = poll_interval_seconds.min(expires_in_seconds);
        Ok(PreparedDeviceCodeSession {
            id: random_urlsafe()?,
            expires_in_seconds,
            poll_interval_seconds,
            session: Self {
                provider,
                device_code: SecretString::from(device_code.to_owned()),
                poll_interval_seconds,
                next_poll_at: now,
                expires_at: now + std::time::Duration::from_secs(expires_in_seconds),
            },
        })
    }

    pub(super) fn device_code(&self) -> &str {
        self.device_code.expose_secret()
    }

    pub(super) fn retry_after(&self, now: Instant) -> Result<Option<u64>, OAuthError> {
        if self.expires_at <= now {
            return Err(OAuthError::SessionExpired);
        }
        Ok((self.next_poll_at > now).then(|| {
            let remaining = self.next_poll_at.duration_since(now);
            remaining
                .as_secs()
                .saturating_add(u64::from(remaining.subsec_nanos() > 0))
        }))
    }

    pub(super) fn defer(&mut self, now: Instant, slow_down: bool) -> Result<u64, OAuthError> {
        if self.expires_at <= now {
            return Err(OAuthError::SessionExpired);
        }
        if slow_down {
            self.poll_interval_seconds = self
                .poll_interval_seconds
                .saturating_add(DEVICE_SLOW_DOWN_SECONDS)
                .min(MAX_DEVICE_SESSION_TTL_SECONDS);
        }
        let remaining = self.expires_at.duration_since(now).as_secs().max(1);
        let retry_after = self.poll_interval_seconds.min(remaining);
        self.next_poll_at = now + std::time::Duration::from_secs(retry_after);
        Ok(retry_after)
    }
}

enum OAuthSession {
    AuthorizationCode(AuthorizationCodeSession),
    DeviceCode(DeviceCodeSession),
}

impl OAuthSession {
    const fn expires_at(&self) -> Instant {
        match self {
            Self::AuthorizationCode(session) => session.expires_at,
            Self::DeviceCode(session) => session.expires_at,
        }
    }
}

#[derive(Default)]
pub(super) struct OAuthSessionStore {
    sessions: HashMap<String, OAuthSession>,
}

impl OAuthSessionStore {
    pub(super) fn insert_authorization_code(
        &mut self,
        id: String,
        session: AuthorizationCodeSession,
        now: Instant,
    ) -> Result<(), OAuthError> {
        self.insert(id, OAuthSession::AuthorizationCode(session), now)
    }

    pub(super) fn insert_device_code(
        &mut self,
        id: String,
        session: DeviceCodeSession,
        now: Instant,
    ) -> Result<(), OAuthError> {
        self.insert(id, OAuthSession::DeviceCode(session), now)
    }

    fn insert(
        &mut self,
        id: String,
        session: OAuthSession,
        now: Instant,
    ) -> Result<(), OAuthError> {
        self.sessions
            .retain(|_, session| session.expires_at() > now);
        if self.sessions.len() >= MAX_ACTIVE_SESSIONS {
            return Err(OAuthError::SessionCapacity);
        }
        self.sessions.insert(id, session);
        Ok(())
    }

    pub(super) fn take_authorization_code(
        &mut self,
        id: &str,
        now: Instant,
    ) -> Result<AuthorizationCodeSession, OAuthError> {
        self.validate_target(id, now, |session| {
            matches!(session, OAuthSession::AuthorizationCode(_))
        })?;
        let Some(OAuthSession::AuthorizationCode(session)) = self.sessions.remove(id) else {
            unreachable!("validated authorization-code session kind")
        };
        Ok(session)
    }

    pub(super) fn take_device_code(
        &mut self,
        id: &str,
        now: Instant,
    ) -> Result<DeviceCodeSession, OAuthError> {
        self.validate_target(id, now, |session| {
            matches!(session, OAuthSession::DeviceCode(_))
        })?;
        let Some(OAuthSession::DeviceCode(session)) = self.sessions.remove(id) else {
            unreachable!("validated device-code session kind")
        };
        Ok(session)
    }

    pub(super) fn restore_device_code(&mut self, id: String, session: DeviceCodeSession) {
        self.sessions.insert(id, OAuthSession::DeviceCode(session));
    }

    fn validate_target(
        &mut self,
        id: &str,
        now: Instant,
        expected: impl FnOnce(&OAuthSession) -> bool,
    ) -> Result<(), OAuthError> {
        let session = self.sessions.get(id).ok_or(OAuthError::InvalidSession)?;
        if session.expires_at() <= now {
            self.sessions.remove(id);
            return Err(OAuthError::SessionExpired);
        }
        if !expected(session) {
            return Err(OAuthError::InvalidSession);
        }
        Ok(())
    }
}

fn random_urlsafe() -> Result<String, OAuthError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|_| OAuthError::RandomGeneration)?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}
