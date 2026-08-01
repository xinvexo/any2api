use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex as StdMutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use any2api_domain::{ConfigRevision, OAuthAccountId};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::OAuthAccountRefresh;
use any2api_transport::api::TransportManager;
use futures_util::{StreamExt, stream};
use tokio::sync::Mutex;

use crate::{configuration::ConfigPublisher, lifecycle::ProcessLifecycle};

use super::account::{PreparedOAuthRefresh, is_due};

const SCHEDULED_REFRESH_CONCURRENCY: usize = 6;

pub(crate) struct OAuthRefresher {
    pub(super) providers: Arc<ProviderRegistry>,
    pub(super) transport: Arc<dyn TransportManager>,
    pub(super) publisher: Arc<ConfigPublisher>,
    pub(super) gates: StdMutex<HashMap<OAuthAccountId, Weak<Mutex<()>>>>,
    worker_started: AtomicBool,
}

impl OAuthRefresher {
    pub(in crate::oauth) fn new(
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

    pub(in crate::oauth) fn start(self: &Arc<Self>, lifecycle: &ProcessLifecycle) -> bool {
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
            let outcome = self.scan_due_accounts().await;
            let latest_revision = *revisions.borrow_and_update();
            if outcome.requires_immediate_rescan(latest_revision) {
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

    pub(super) async fn scan_due_accounts(&self) -> ScanOutcome {
        let snapshot = self.publisher.current_snapshot();
        let started_at = snapshot.revision();
        let lead_time = snapshot.settings().oauth().refresh_lead_time_secs();
        let due = snapshot
            .oauth_accounts()
            .accounts()
            .iter()
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

        let prepared = stream::iter(due)
            .map(|(id, token_version)| async move {
                (
                    id,
                    token_version,
                    self.prepare_scheduled_refresh(id, token_version).await,
                )
            })
            .buffer_unordered(SCHEDULED_REFRESH_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;
        let published = self.publish_prepared_refreshes(prepared).await;
        ScanOutcome {
            started_at,
            published,
        }
    }

    async fn publish_prepared_refreshes(
        &self,
        prepared: Vec<(
            OAuthAccountId,
            u64,
            Result<Option<PreparedOAuthRefresh>, super::account::OAuthRefreshError>,
        )>,
    ) -> Option<ConfigRevision> {
        let mut refreshes = Vec::new();
        let mut pending = Vec::new();
        let mut gates = Vec::new();
        for (id, _token_version, result) in prepared {
            match result {
                Ok(Some(prepared)) => {
                    let (id, token_version, email, expires_at, document, gate) =
                        prepared.into_parts();
                    refreshes.push(OAuthAccountRefresh::new(
                        id,
                        token_version,
                        email,
                        expires_at,
                        document,
                    ));
                    pending.push((id, token_version));
                    gates.push(gate);
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    oauth_account_id = %id,
                    error = %error,
                    "OAuth account token refresh failed"
                ),
            }
        }
        if refreshes.is_empty() {
            return None;
        }

        match self
            .publisher
            .refresh_oauth_accounts(refreshes, gates)
            .await
        {
            Ok((snapshot, changed)) => {
                for (id, observed_token_version) in pending {
                    if let Some(next_version) = snapshot
                        .oauth_accounts()
                        .get(id)
                        .map(|account| account.token_version())
                        .filter(|version| *version > observed_token_version)
                    {
                        tracing::info!(
                            oauth_account_id = %id,
                            token_version = next_version,
                            "OAuth account token refreshed"
                        );
                    }
                }
                changed.then(|| snapshot.revision())
            }
            Err(error) => {
                tracing::warn!(
                    prepared_account_count = pending.len(),
                    error = %error,
                    "OAuth account refresh batch publication failed"
                );
                None
            }
        }
    }

    fn retain_active_gates(&self, active: HashSet<OAuthAccountId>) {
        self.gates
            .lock()
            .expect("OAuth refresh gate lock poisoned")
            .retain(|id, gate| active.contains(id) || gate.strong_count() > 0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScanOutcome {
    started_at: ConfigRevision,
    published: Option<ConfigRevision>,
}

impl ScanOutcome {
    fn requires_immediate_rescan(self, latest: ConfigRevision) -> bool {
        match self.published {
            Some(published) => {
                let own_next = self
                    .started_at
                    .checked_next()
                    .expect("a successful publication proves the revision can advance");
                published > own_next || latest > published
            }
            None => latest > self.started_at,
        }
    }
}
