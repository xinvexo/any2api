use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_domain::AdminSettings;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock, Semaphore};

use super::{
    AdminCredentialStore, AdminCredentialStoreError, AdminSessionIssue, AuthenticatedAdminSession,
    password::{hash_password, validate_new_password, verify_password},
    session::{SessionKey, SessionRecord, decode, random_bytes},
};

pub struct AdminAuthService {
    store: Arc<dyn AdminCredentialStore>,
    password_hash: RwLock<Option<String>>,
    setup_token: RwLock<Option<[u8; 32]>>,
    setup_lock: Mutex<()>,
    sessions: Mutex<HashMap<SessionKey, SessionRecord>>,
    failures: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
    password_checks: Arc<Semaphore>,
    setup_checks: Arc<Semaphore>,
}

const MAX_FAILURE_SOURCES: usize = 1_024;
const MAX_CONCURRENT_PASSWORD_CHECKS: usize = 4;

impl AdminAuthService {
    pub async fn load(store: Arc<dyn AdminCredentialStore>) -> Result<Self, AdminAuthError> {
        let password_hash = store.load().await.map_err(AdminAuthError::Store)?;
        let setup_token = password_hash.is_none().then(random_bytes).transpose()?;
        Ok(Self {
            store,
            password_hash: RwLock::new(password_hash.map(|value| value.as_str().to_owned())),
            setup_token: RwLock::new(setup_token),
            setup_lock: Mutex::new(()),
            sessions: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
            password_checks: Arc::new(Semaphore::new(MAX_CONCURRENT_PASSWORD_CHECKS)),
            setup_checks: Arc::new(Semaphore::new(1)),
        })
    }

    pub async fn is_initialized(&self) -> bool {
        self.password_hash.read().await.is_some()
    }

    pub async fn initialize_if_missing(&self, password: String) -> Result<bool, AdminAuthError> {
        let _guard = self.setup_lock.lock().await;
        if self.is_initialized().await {
            return Ok(false);
        }
        self.initialize_locked(password).await
    }

    pub async fn initialize_with_setup_token(
        &self,
        password: String,
        setup_token: &str,
    ) -> Result<bool, AdminAuthError> {
        let _guard = self.setup_lock.lock().await;
        if self.is_initialized().await {
            return Ok(false);
        }
        let provided = decode(setup_token).ok_or(AdminAuthError::InvalidSetupToken)?;
        let expected = self
            .setup_token
            .read()
            .await
            .ok_or(AdminAuthError::InvalidSetupToken)?;
        if !bool::from(expected.ct_eq(&provided)) {
            return Err(AdminAuthError::InvalidSetupToken);
        }
        self.initialize_locked(password).await
    }

    pub async fn setup_token(&self) -> Option<String> {
        self.setup_token.read().await.map(super::session::encode)
    }

    async fn initialize_locked(&self, password: String) -> Result<bool, AdminAuthError> {
        validate_new_password(&password)?;
        let password_check = Arc::clone(&self.setup_checks)
            .try_acquire_owned()
            .map_err(|_| AdminAuthError::RateLimited { retry_after: 1 })?;

        let password_hash = hash_password(password, password_check).await?;
        if self
            .store
            .initialize(&password_hash)
            .await
            .map_err(AdminAuthError::Store)?
        {
            *self.password_hash.write().await = Some(password_hash);
            *self.setup_token.write().await = None;
            return Ok(true);
        }

        let stored = self
            .store
            .load()
            .await
            .map_err(AdminAuthError::Store)?
            .ok_or(AdminAuthError::PasswordHash)?;
        *self.password_hash.write().await = Some(stored.as_str().to_owned());
        *self.setup_token.write().await = None;
        Ok(false)
    }

    pub async fn login(
        &self,
        password: String,
        source: IpAddr,
        settings: &AdminSettings,
    ) -> Result<AdminSessionIssue, AdminAuthError> {
        self.ensure_login_allowed(source, settings).await?;
        let password_hash = self
            .password_hash
            .read()
            .await
            .clone()
            .ok_or(AdminAuthError::NotInitialized)?;
        let password_check = Arc::clone(&self.password_checks)
            .try_acquire_owned()
            .map_err(|_| AdminAuthError::RateLimited { retry_after: 1 })?;
        self.record_failure(source, settings).await;
        if !verify_password(password_hash, password, password_check).await? {
            return Err(AdminAuthError::InvalidCredentials);
        }
        self.failures.lock().await.remove(&source);
        self.issue_session().await
    }

    pub async fn authenticate(
        &self,
        token: &str,
        settings: &AdminSettings,
    ) -> Option<AuthenticatedAdminSession> {
        let key = SessionKey(decode(token)?);
        let now = Instant::now();
        let mut sessions = self.sessions.lock().await;
        let authenticated = sessions
            .get_mut(&key)
            .and_then(|record| record.authenticate(key, now, settings));
        if authenticated.is_none() {
            sessions.remove(&key);
        }
        authenticated
    }

    pub async fn logout(&self, session: AuthenticatedAdminSession) {
        self.sessions.lock().await.remove(&session.key);
    }

    #[cfg(test)]
    pub(super) fn available_password_checks(&self) -> usize {
        self.password_checks.available_permits()
    }

    #[cfg(test)]
    pub(super) fn available_setup_checks(&self) -> usize {
        self.setup_checks.available_permits()
    }

    #[cfg(test)]
    pub(super) async fn failure_count(&self, source: IpAddr) -> usize {
        self.failures
            .lock()
            .await
            .get(&source)
            .map_or(0, VecDeque::len)
    }

    async fn issue_session(&self) -> Result<AdminSessionIssue, AdminAuthError> {
        let token = random_bytes()?;
        let csrf = random_bytes()?;
        let key = SessionKey(token);
        self.sessions
            .lock()
            .await
            .insert(key, SessionRecord::new(csrf, Instant::now()));
        Ok(AdminSessionIssue::new(token, csrf))
    }

    async fn ensure_login_allowed(
        &self,
        source: IpAddr,
        settings: &AdminSettings,
    ) -> Result<(), AdminAuthError> {
        let now = Instant::now();
        let window = Duration::from_millis(settings.login_failure_window_ms());
        let mut failures = self.failures.lock().await;
        prune_failure_sources(&mut failures, now, window);
        let entries = failures.entry(source).or_default();
        prune_failures(entries, now, window);
        if entries.len() < settings.login_max_failures() as usize {
            return Ok(());
        }
        let retry_after = entries
            .front()
            .and_then(|first| first.checked_add(window))
            .and_then(|deadline| deadline.checked_duration_since(now))
            .map(|duration| duration.as_secs().max(1))
            .unwrap_or(1);
        Err(AdminAuthError::RateLimited { retry_after })
    }

    async fn record_failure(&self, source: IpAddr, settings: &AdminSettings) {
        let now = Instant::now();
        let window = Duration::from_millis(settings.login_failure_window_ms());
        let mut failures = self.failures.lock().await;
        prune_failure_sources(&mut failures, now, window);
        if !failures.contains_key(&source) && failures.len() >= MAX_FAILURE_SOURCES {
            remove_oldest_failure_source(&mut failures);
        }
        let entries = failures.entry(source).or_default();
        prune_failures(entries, now, window);
        entries.push_back(now);
    }
}

fn prune_failures(entries: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while entries
        .front()
        .is_some_and(|failure| now.duration_since(*failure) >= window)
    {
        entries.pop_front();
    }
}

fn prune_failure_sources(
    failures: &mut HashMap<IpAddr, VecDeque<Instant>>,
    now: Instant,
    window: Duration,
) {
    failures.retain(|_, entries| {
        prune_failures(entries, now, window);
        !entries.is_empty()
    });
}

fn remove_oldest_failure_source(failures: &mut HashMap<IpAddr, VecDeque<Instant>>) {
    let oldest = failures
        .iter()
        .filter_map(|(source, entries)| entries.back().map(|latest| (*source, *latest)))
        .min_by_key(|(_, latest)| *latest)
        .map(|(source, _)| source);
    if let Some(source) = oldest {
        failures.remove(&source);
    }
}

#[derive(Debug, Error)]
pub enum AdminAuthError {
    #[error("administrator credential storage failed")]
    Store(#[source] AdminCredentialStoreError),
    #[error("administrator password must contain between 12 and 1024 bytes")]
    InvalidPassword,
    #[error("administrator setup token is invalid")]
    InvalidSetupToken,
    #[error("administrator password hash is invalid")]
    PasswordHash,
    #[error("administrator password task failed")]
    PasswordTask,
    #[error("secure random generation failed")]
    Random,
    #[error("administrator password is not initialized")]
    NotInitialized,
    #[error("administrator credentials are invalid")]
    InvalidCredentials,
    #[error("administrator login is rate limited")]
    RateLimited { retry_after: u64 },
}

impl AdminAuthError {
    pub const fn retry_after(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after } => Some(*retry_after),
            _ => None,
        }
    }
}
