use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex as StdMutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use any2api_domain::OAuthAccountId;
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;
use futures_util::{StreamExt, stream};
use tokio::sync::Mutex;

use crate::{configuration::ConfigPublisher, lifecycle::ProcessLifecycle};

use super::account::is_due;

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

    fn retain_active_gates(&self, active: HashSet<OAuthAccountId>) {
        self.gates
            .lock()
            .expect("OAuth refresh gate lock poisoned")
            .retain(|id, gate| active.contains(id) || gate.strong_count() > 0);
    }
}
