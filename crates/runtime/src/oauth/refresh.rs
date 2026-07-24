use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex as StdMutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use any2api_domain::OAuthAccountId;
use any2api_provider::api::{OAuthGrant, ProviderRegistry};
use any2api_transport::api::TransportManager;
use futures_util::{StreamExt, stream};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
    config_publish_error::ConfigPublishError, process_lifecycle::ProcessLifecycle,
    publisher::ConfigPublisher,
};

use super::{document, error::OAuthError, token_request};

pub(crate) struct OAuthRefresher {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    publisher: Arc<ConfigPublisher>,
    gates: StdMutex<HashMap<OAuthAccountId, Weak<Mutex<()>>>>,
    worker_started: AtomicBool,
}

impl OAuthRefresher {
    pub(super) fn new(
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
        publisher: Arc<ConfigPublisher>,
    ) -> Arc<Self> {
        Arc::new(Self {
            providers,
            transport,
            publisher,
            gates: StdMutex::new(HashMap::new()),
            worker_started: AtomicBool::new(false),
        })
    }

    pub(super) fn start(self: &Arc<Self>, lifecycle: &ProcessLifecycle) -> bool {
        if self
            .worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        let refresher = Arc::clone(self);
        drop(lifecycle.spawn_until_draining(async move {
            refresher.run().await;
        }));
        true
    }

    async fn run(self: Arc<Self>) {
        let mut revisions = self.publisher.subscribe_revision();
        loop {
            revisions.borrow_and_update();
            self.scan_due_accounts().await;
            if revisions.has_changed().unwrap_or(false) {
                continue;
            }
            let interval = Duration::from_secs(
                self.publisher
                    .current_snapshot()
                    .settings()
                    .oauth()
                    .refresh_scan_interval_secs(),
            );
            tokio::select! {
                () = tokio::time::sleep(interval) => {}
                changed = revisions.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
            }
        }
    }

    async fn scan_due_accounts(&self) {
        let snapshot = self.publisher.current_snapshot();
        let lead_time = snapshot.settings().oauth().refresh_lead_time_secs();
        let due = snapshot
            .oauth_accounts()
            .accounts()
            .iter()
            .filter(|account| account.enabled())
            .filter(|account| is_due(account.expires_at(), lead_time))
            .map(|account| (account.id(), account.token_version()))
            .collect::<Vec<_>>();
        self.retain_active_gates(
            snapshot
                .oauth_accounts()
                .accounts()
                .iter()
                .map(|account| account.id())
                .collect(),
        );
        drop(snapshot);

        stream::iter(due)
            .for_each_concurrent(None, |(id, token_version)| async move {
                match self.refresh_if_due(id, token_version).await {
                    Ok(Some(next_version)) => tracing::info!(
                        oauth_account_id = %id,
                        token_version = next_version,
                        "OAuth account token refreshed"
                    ),
                    Ok(None) => {}
                    Err(error) => tracing::warn!(
                        oauth_account_id = %id,
                        error = %error,
                        "OAuth account token refresh failed"
                    ),
                }
            })
            .await;
    }

    pub(super) async fn refresh_if_due(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
    ) -> Result<Option<u64>, OAuthRefreshError> {
        let result = self
            .refresh(id, observed_token_version, RefreshTrigger::Scheduled)
            .await?;
        Ok(match result {
            RefreshResult::Refreshed(snapshot) => snapshot
                .oauth_accounts()
                .get(id)
                .map(|account| account.token_version()),
            RefreshResult::AlreadyUpdated(_) | RefreshResult::Unavailable => None,
        })
    }

    pub(crate) async fn refresh_after_authentication_failure(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
    ) -> Option<Arc<crate::published_snapshot::PublishedSnapshot>> {
        match self
            .refresh(
                id,
                observed_token_version,
                RefreshTrigger::AuthenticationFailure,
            )
            .await
        {
            Ok(RefreshResult::Refreshed(snapshot)) => {
                tracing::info!(oauth_account_id = %id, "OAuth account refreshed after authentication failure");
                Some(snapshot)
            }
            Ok(RefreshResult::AlreadyUpdated(snapshot)) => Some(snapshot),
            Ok(RefreshResult::Unavailable) => None,
            Err(error) => {
                tracing::warn!(
                    oauth_account_id = %id,
                    error = %error,
                    "OAuth account refresh after authentication failure failed"
                );
                None
            }
        }
    }

    async fn refresh(
        &self,
        id: OAuthAccountId,
        observed_token_version: u64,
        trigger: RefreshTrigger,
    ) -> Result<RefreshResult, OAuthRefreshError> {
        let gate = self.gate(id);
        let (_guard, waited_for_flight) = match gate.try_lock() {
            Ok(guard) => (guard, false),
            Err(_) => (gate.lock().await, true),
        };
        let snapshot = self.publisher.current_snapshot();
        let Some(account) = snapshot.oauth_accounts().get(id) else {
            return Ok(RefreshResult::Unavailable);
        };
        let lead_time = snapshot.settings().oauth().refresh_lead_time_secs();
        if account.token_version() != observed_token_version {
            return Ok(if account.token_version() > observed_token_version {
                RefreshResult::AlreadyUpdated(snapshot)
            } else {
                RefreshResult::Unavailable
            });
        }
        if waited_for_flight {
            return Ok(RefreshResult::Unavailable);
        }
        if !account.enabled()
            || (trigger == RefreshTrigger::Scheduled && !is_due(account.expires_at(), lead_time))
        {
            return Ok(RefreshResult::Unavailable);
        }
        let token = snapshot
            .oauth_token_material(id)
            .ok_or(OAuthRefreshError::TokenMaterialUnavailable)?;
        let Some(refresh_token) = token.refresh_token() else {
            tracing::debug!(oauth_account_id = %id, "OAuth account has no refresh token");
            return Ok(RefreshResult::Unavailable);
        };
        let driver = self
            .providers
            .get(account.provider_kind())
            .ok_or(OAuthRefreshError::ProviderUnavailable)?;
        let plan = driver
            .oauth_token_request(OAuthGrant::RefreshToken, refresh_token, None, None)
            .map_err(OAuthError::Provider)?;
        let proxy = snapshot
            .resolved_transport_proxy_for_oauth_account()
            .ok_or(OAuthError::PublishedProxyUnavailable)?;
        let strict_ssrf = snapshot.settings().upstream().strict_ssrf();
        let body =
            token_request::execute(self.transport.as_ref(), proxy, strict_ssrf, plan).await?;
        let refreshed = driver
            .parse_oauth_refresh_token(&body, token.as_ref())
            .map_err(OAuthError::from_token_response_error)?;
        if refreshed.provider() != account.provider_kind() {
            return Err(OAuthError::TokenResponseInvalid.into());
        }
        driver
            .oauth_routing_profile(&refreshed)
            .map_err(OAuthError::Provider)?;
        let document = document::serialize(&refreshed)?;
        let safe_account_email = refreshed.email().map(str::to_owned);
        let expires_at = refreshed.expires_at();
        drop(snapshot);

        let published = match self
            .publisher
            .refresh_oauth_account(
                id,
                observed_token_version,
                safe_account_email,
                expires_at,
                document,
            )
            .await
        {
            Ok(published) => published,
            Err(
                ConfigPublishError::OAuthAccountNotFound
                | ConfigPublishError::OAuthAccountTokenVersionConflict,
            ) => {
                let current = self.publisher.current_snapshot();
                return Ok(match current.oauth_accounts().get(id) {
                    Some(account) if account.token_version() > observed_token_version => {
                        RefreshResult::AlreadyUpdated(current)
                    }
                    _ => RefreshResult::Unavailable,
                });
            }
            Err(error) => return Err(OAuthRefreshError::Publish(error)),
        };
        published
            .oauth_accounts()
            .get(id)
            .filter(|account| account.token_version() > observed_token_version)
            .ok_or(OAuthRefreshError::AccountUnavailable)?;
        Ok(RefreshResult::Refreshed(published))
    }

    fn gate(&self, id: OAuthAccountId) -> Arc<Mutex<()>> {
        let mut gates = self.gates.lock().expect("OAuth refresh gate lock poisoned");
        if let Some(gate) = gates.get(&id).and_then(Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(Mutex::new(()));
        gates.insert(id, Arc::downgrade(&gate));
        gate
    }

    fn retain_active_gates(&self, active: HashSet<OAuthAccountId>) {
        self.gates
            .lock()
            .expect("OAuth refresh gate lock poisoned")
            .retain(|id, gate| active.contains(id) || gate.strong_count() > 0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RefreshTrigger {
    Scheduled,
    AuthenticationFailure,
}

enum RefreshResult {
    Refreshed(Arc<crate::published_snapshot::PublishedSnapshot>),
    AlreadyUpdated(Arc<crate::published_snapshot::PublishedSnapshot>),
    Unavailable,
}

fn is_due(expires_at: Option<i64>, lead_time_secs: u64) -> bool {
    let Some(expires_at) = expires_at else {
        return false;
    };
    let lead_time = i64::try_from(lead_time_secs).expect("validated OAuth lead time fits i64");
    document::unix_now() >= expires_at.saturating_sub(lead_time)
}

#[derive(Debug, Error)]
pub(super) enum OAuthRefreshError {
    #[error("OAuth account disappeared after refresh publication")]
    AccountUnavailable,
    #[error("OAuth provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth token material is unavailable")]
    TokenMaterialUnavailable,
    #[error(transparent)]
    OAuth(#[from] OAuthError),
    #[error("OAuth refresh publication failed")]
    Publish(#[source] ConfigPublishError),
}
