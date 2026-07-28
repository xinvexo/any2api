use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use crate::oauth::error::OAuthError;

use super::{AuthorizationCodeSession, DeviceCodeSession, MAX_ACTIVE_SESSIONS};

pub(in crate::oauth) enum DevicePollAcquisition {
    Ready(DevicePollLease),
    Pending { retry_after_seconds: u64 },
}

#[derive(Clone, Default)]
pub(in crate::oauth) struct OAuthSessionRegistry {
    inner: Arc<Mutex<OAuthSessionStore>>,
}

impl OAuthSessionRegistry {
    pub(in crate::oauth) fn insert_authorization_code(
        &self,
        id: String,
        session: AuthorizationCodeSession,
        now: Instant,
    ) -> Result<(), OAuthError> {
        lock(&self.inner).insert(id, OAuthSession::AuthorizationCode(session), now)
    }

    pub(in crate::oauth) fn insert_device_code(
        &self,
        id: String,
        session: DeviceCodeSession,
        now: Instant,
    ) -> Result<(), OAuthError> {
        lock(&self.inner).insert(
            id,
            OAuthSession::DeviceCode(DeviceSessionState::Ready(session)),
            now,
        )
    }

    pub(in crate::oauth) fn take_authorization_code(
        &self,
        id: &str,
        now: Instant,
    ) -> Result<AuthorizationCodeSession, OAuthError> {
        lock(&self.inner).take_authorization_code(id, now)
    }

    pub(in crate::oauth) fn acquire_device_poll(
        &self,
        id: &str,
        now: Instant,
    ) -> Result<DevicePollAcquisition, OAuthError> {
        match lock(&self.inner).acquire_device_poll(id, now)? {
            StorePollAcquisition::Ready {
                generation,
                session,
            } => Ok(DevicePollAcquisition::Ready(DevicePollLease {
                inner: Arc::clone(&self.inner),
                id: id.to_owned(),
                generation,
                session: Some(session),
            })),
            StorePollAcquisition::Pending {
                retry_after_seconds,
            } => Ok(DevicePollAcquisition::Pending {
                retry_after_seconds,
            }),
        }
    }

    #[cfg(test)]
    pub(in crate::oauth) fn active_len(&self) -> usize {
        lock(&self.inner).sessions.len()
    }
}

pub(in crate::oauth) struct DevicePollLease {
    inner: Arc<Mutex<OAuthSessionStore>>,
    id: String,
    generation: u64,
    session: Option<DeviceCodeSession>,
}

impl DevicePollLease {
    pub(in crate::oauth) fn provider(&self) -> any2api_domain::ProviderKind {
        self.session().provider
    }

    pub(in crate::oauth) fn device_code(&self) -> &str {
        self.session().device_code()
    }

    pub(in crate::oauth) fn restore(mut self, slow_down: bool) -> Result<u64, OAuthError> {
        let mut session = self.session.take().expect("active device poll lease");
        match session.defer(Instant::now(), slow_down) {
            Ok(retry_after_seconds) => {
                lock(&self.inner).restore_device_poll(&self.id, self.generation, session);
                Ok(retry_after_seconds)
            }
            Err(error) => {
                lock(&self.inner).finish_device_poll(&self.id, self.generation);
                Err(error)
            }
        }
    }

    pub(in crate::oauth) fn finish(mut self) {
        self.session.take();
        lock(&self.inner).finish_device_poll(&self.id, self.generation);
    }

    fn session(&self) -> &DeviceCodeSession {
        self.session.as_ref().expect("active device poll lease")
    }
}

impl Drop for DevicePollLease {
    fn drop(&mut self) {
        let Some(mut session) = self.session.take() else {
            return;
        };
        let mut store = lock(&self.inner);
        if session.defer(Instant::now(), false).is_ok() {
            store.restore_device_poll(&self.id, self.generation, session);
        } else {
            store.finish_device_poll(&self.id, self.generation);
        }
    }
}

#[derive(Default)]
struct OAuthSessionStore {
    sessions: HashMap<String, OAuthSession>,
    next_poll_generation: u64,
}

enum OAuthSession {
    AuthorizationCode(AuthorizationCodeSession),
    DeviceCode(DeviceSessionState),
}

enum DeviceSessionState {
    Ready(DeviceCodeSession),
    Polling {
        generation: u64,
        expires_at: Instant,
    },
}

enum StorePollAcquisition {
    Ready {
        generation: u64,
        session: DeviceCodeSession,
    },
    Pending {
        retry_after_seconds: u64,
    },
}

impl OAuthSessionStore {
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

    fn take_authorization_code(
        &mut self,
        id: &str,
        now: Instant,
    ) -> Result<AuthorizationCodeSession, OAuthError> {
        let session = self.sessions.remove(id).ok_or(OAuthError::InvalidSession)?;
        if session.expires_at() <= now {
            return Err(OAuthError::SessionExpired);
        }
        match session {
            OAuthSession::AuthorizationCode(session) => Ok(session),
            other => {
                self.sessions.insert(id.to_owned(), other);
                Err(OAuthError::InvalidSession)
            }
        }
    }

    fn acquire_device_poll(
        &mut self,
        id: &str,
        now: Instant,
    ) -> Result<StorePollAcquisition, OAuthError> {
        let session = self.sessions.remove(id).ok_or(OAuthError::InvalidSession)?;
        if session.expires_at() <= now {
            return Err(OAuthError::SessionExpired);
        }
        match session {
            OAuthSession::DeviceCode(DeviceSessionState::Ready(session)) => {
                if let Some(retry_after_seconds) = session.retry_after(now)? {
                    self.sessions.insert(
                        id.to_owned(),
                        OAuthSession::DeviceCode(DeviceSessionState::Ready(session)),
                    );
                    return Ok(StorePollAcquisition::Pending {
                        retry_after_seconds,
                    });
                }
                self.next_poll_generation = self.next_poll_generation.wrapping_add(1);
                let generation = self.next_poll_generation;
                self.sessions.insert(
                    id.to_owned(),
                    OAuthSession::DeviceCode(DeviceSessionState::Polling {
                        generation,
                        expires_at: session.expires_at(),
                    }),
                );
                Ok(StorePollAcquisition::Ready {
                    generation,
                    session,
                })
            }
            OAuthSession::DeviceCode(state @ DeviceSessionState::Polling { .. }) => {
                self.sessions
                    .insert(id.to_owned(), OAuthSession::DeviceCode(state));
                Ok(StorePollAcquisition::Pending {
                    retry_after_seconds: 1,
                })
            }
            other => {
                self.sessions.insert(id.to_owned(), other);
                Err(OAuthError::InvalidSession)
            }
        }
    }

    fn restore_device_poll(&mut self, id: &str, generation: u64, session: DeviceCodeSession) {
        if self.poll_generation(id) == Some(generation) {
            self.sessions.insert(
                id.to_owned(),
                OAuthSession::DeviceCode(DeviceSessionState::Ready(session)),
            );
        }
    }

    fn finish_device_poll(&mut self, id: &str, generation: u64) {
        if self.poll_generation(id) == Some(generation) {
            self.sessions.remove(id);
        }
    }

    fn poll_generation(&self, id: &str) -> Option<u64> {
        match self.sessions.get(id) {
            Some(OAuthSession::DeviceCode(DeviceSessionState::Polling { generation, .. })) => {
                Some(*generation)
            }
            _ => None,
        }
    }
}

impl OAuthSession {
    const fn expires_at(&self) -> Instant {
        match self {
            Self::AuthorizationCode(session) => session.expires_at,
            Self::DeviceCode(DeviceSessionState::Ready(session)) => session.expires_at(),
            Self::DeviceCode(DeviceSessionState::Polling { expires_at, .. }) => *expires_at,
        }
    }
}

fn lock(store: &Mutex<OAuthSessionStore>) -> MutexGuard<'_, OAuthSessionStore> {
    store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
