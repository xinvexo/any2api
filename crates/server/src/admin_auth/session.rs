use std::{collections::HashMap, time::Instant};

use any2api_domain::AdminSettings;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use subtle::ConstantTimeEq;

use super::AdminAuthError;

pub(super) const TOKEN_BYTES: usize = 32;
pub(super) const MAX_ACTIVE_ADMIN_SESSIONS: usize = 32;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(super) struct SessionKey(pub(super) [u8; TOKEN_BYTES]);

pub(super) struct SessionRecord {
    csrf: [u8; TOKEN_BYTES],
    created_at: Instant,
    last_seen_at: Instant,
}

impl SessionRecord {
    pub(super) fn new(csrf: [u8; TOKEN_BYTES], now: Instant) -> Self {
        Self {
            csrf,
            created_at: now,
            last_seen_at: now,
        }
    }

    pub(super) fn authenticate(
        &mut self,
        key: SessionKey,
        now: Instant,
        settings: &AdminSettings,
    ) -> Option<AuthenticatedAdminSession> {
        if self.is_expired(now, settings) {
            return None;
        }
        self.last_seen_at = now;
        Some(AuthenticatedAdminSession {
            key,
            csrf: self.csrf,
        })
    }

    fn is_expired(&self, now: Instant, settings: &AdminSettings) -> bool {
        now.duration_since(self.created_at).as_secs() >= settings.session_absolute_timeout_secs()
            || now.duration_since(self.last_seen_at).as_secs()
                >= settings.session_idle_timeout_secs()
    }

    const fn created_at(&self) -> Instant {
        self.created_at
    }
}

#[derive(Default)]
pub(super) struct AdminSessionStore {
    sessions: HashMap<SessionKey, SessionRecord>,
}

impl AdminSessionStore {
    pub(super) fn issue(
        &mut self,
        key: SessionKey,
        csrf: [u8; TOKEN_BYTES],
        now: Instant,
        settings: &AdminSettings,
    ) {
        self.prune_expired(now, settings);
        if self.sessions.len() >= MAX_ACTIVE_ADMIN_SESSIONS {
            self.remove_oldest();
        }
        self.sessions.insert(key, SessionRecord::new(csrf, now));
    }

    pub(super) fn authenticate(
        &mut self,
        key: SessionKey,
        now: Instant,
        settings: &AdminSettings,
    ) -> Option<AuthenticatedAdminSession> {
        self.prune_expired(now, settings);
        self.sessions
            .get_mut(&key)
            .and_then(|record| record.authenticate(key, now, settings))
    }

    pub(super) fn remove(&mut self, key: SessionKey) {
        self.sessions.remove(&key);
    }

    pub(super) fn clear(&mut self) {
        self.sessions.clear();
    }

    pub(super) fn replace_with(&mut self, key: SessionKey, csrf: [u8; TOKEN_BYTES], now: Instant) {
        self.sessions.clear();
        self.sessions.insert(key, SessionRecord::new(csrf, now));
    }

    fn prune_expired(&mut self, now: Instant, settings: &AdminSettings) {
        self.sessions
            .retain(|_, record| !record.is_expired(now, settings));
    }

    fn remove_oldest(&mut self) {
        let oldest = self
            .sessions
            .iter()
            .min_by_key(|(_, record)| record.created_at())
            .map(|(key, _)| *key);
        if let Some(key) = oldest {
            self.sessions.remove(&key);
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.sessions.len()
    }
}

#[derive(Clone, Copy)]
pub struct AuthenticatedAdminSession {
    pub(super) key: SessionKey,
    csrf: [u8; TOKEN_BYTES],
}

impl AuthenticatedAdminSession {
    #[must_use]
    pub fn csrf_token(self) -> String {
        encode(self.csrf)
    }

    #[must_use]
    pub fn csrf_matches(self, candidate: &str) -> bool {
        decode(candidate).is_some_and(|candidate| bool::from(self.csrf.ct_eq(&candidate)))
    }
}

pub struct AdminSessionIssue {
    token: String,
    csrf_token: String,
}

impl AdminSessionIssue {
    pub(super) fn new(token: [u8; TOKEN_BYTES], csrf: [u8; TOKEN_BYTES]) -> Self {
        Self {
            token: encode(token),
            csrf_token: encode(csrf),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn csrf_token(&self) -> &str {
        &self.csrf_token
    }
}

pub(super) fn random_bytes() -> Result<[u8; TOKEN_BYTES], AdminAuthError> {
    let mut value = [0_u8; TOKEN_BYTES];
    getrandom::fill(&mut value).map_err(|_| AdminAuthError::Random)?;
    Ok(value)
}

pub(super) fn decode(value: &str) -> Option<[u8; TOKEN_BYTES]> {
    let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;
    bytes.try_into().ok()
}

pub(super) fn encode(value: [u8; TOKEN_BYTES]) -> String {
    URL_SAFE_NO_PAD.encode(value)
}

pub(super) fn prepare() -> Result<(SessionKey, [u8; TOKEN_BYTES], AdminSessionIssue), AdminAuthError>
{
    let token = random_bytes()?;
    let csrf = random_bytes()?;
    Ok((SessionKey(token), csrf, AdminSessionIssue::new(token, csrf)))
}

#[cfg(test)]
mod tests;
