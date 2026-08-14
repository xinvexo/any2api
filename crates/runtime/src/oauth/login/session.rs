use std::time::Instant;

use any2api_domain::{OAuthProxySelection, ProviderKind};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use crate::oauth::error::OAuthError;

pub(in crate::oauth) const CALLBACK_SESSION_TTL_SECONDS: u64 = 600;
pub(in crate::oauth) const MAX_DEVICE_SESSION_TTL_SECONDS: u64 = 30 * 60;
const DEVICE_SLOW_DOWN_SECONDS: u64 = 5;
pub(in crate::oauth) const MAX_ACTIVE_SESSIONS: usize = 64;

mod store;

pub(in crate::oauth) use store::{DevicePollAcquisition, OAuthSessionRegistry};

pub(in crate::oauth) struct PreparedAuthorizationCodeSession {
    pub(in crate::oauth) id: String,
    pub(in crate::oauth) state: String,
    pub(in crate::oauth) code_challenge: String,
    pub(in crate::oauth) session: AuthorizationCodeSession,
}

pub(in crate::oauth) struct AuthorizationCodeSession {
    pub(in crate::oauth) provider: ProviderKind,
    pub(in crate::oauth) redirect_uri: &'static str,
    pub(in crate::oauth) proxy_selection: OAuthProxySelection,
    state: SecretString,
    code_verifier: SecretString,
    expires_at: Instant,
}

impl AuthorizationCodeSession {
    pub(in crate::oauth) fn prepare(
        provider: ProviderKind,
        proxy_selection: OAuthProxySelection,
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
                proxy_selection,
                redirect_uri,
                state: SecretString::from(state),
                code_verifier: SecretString::from(code_verifier),
                expires_at: now + std::time::Duration::from_secs(CALLBACK_SESSION_TTL_SECONDS),
            },
        })
    }

    pub(in crate::oauth) fn state(&self) -> &str {
        self.state.expose_secret()
    }

    pub(in crate::oauth) fn code_verifier(&self) -> &str {
        self.code_verifier.expose_secret()
    }
}

pub(in crate::oauth) struct PreparedDeviceCodeSession {
    pub(in crate::oauth) id: String,
    pub(in crate::oauth) expires_in_seconds: u64,
    pub(in crate::oauth) poll_interval_seconds: u64,
    pub(in crate::oauth) session: DeviceCodeSession,
}

pub(in crate::oauth) struct DeviceCodeSession {
    pub(in crate::oauth) provider: ProviderKind,
    proxy_selection: OAuthProxySelection,
    device_code: SecretString,
    poll_interval_seconds: u64,
    next_poll_at: Instant,
    expires_at: Instant,
}

impl DeviceCodeSession {
    pub(in crate::oauth) fn prepare(
        provider: ProviderKind,
        proxy_selection: OAuthProxySelection,
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
                proxy_selection,
                device_code: SecretString::from(device_code.to_owned()),
                poll_interval_seconds,
                next_poll_at: now,
                expires_at: now + std::time::Duration::from_secs(expires_in_seconds),
            },
        })
    }

    pub(in crate::oauth) fn device_code(&self) -> &str {
        self.device_code.expose_secret()
    }

    pub(in crate::oauth) const fn proxy_selection(&self) -> OAuthProxySelection {
        self.proxy_selection
    }

    pub(in crate::oauth) const fn expires_at(&self) -> Instant {
        self.expires_at
    }

    pub(in crate::oauth) fn retry_after(&self, now: Instant) -> Result<Option<u64>, OAuthError> {
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

    pub(in crate::oauth) fn defer(
        &mut self,
        now: Instant,
        slow_down: bool,
    ) -> Result<u64, OAuthError> {
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

fn random_urlsafe() -> Result<String, OAuthError> {
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|_| OAuthError::RandomGeneration)?;
    Ok(URL_SAFE_NO_PAD.encode(random))
}
