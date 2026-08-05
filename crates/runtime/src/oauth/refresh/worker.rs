use std::{
    collections::HashMap,
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

use super::{
    account::{PreparedOAuthRefresh, is_due},
    failure::{OAuthRefreshError, OAuthRefreshFailure, OAuthRefreshTrigger},
    state::OAuthRefreshState,
};

const SCHEDULED_REFRESH_CONCURRENCY: usize = 6;

pub(crate) struct OAuthRefresher {
    pub(super) providers: Arc<ProviderRegistry>,
    pub(super) transport: Arc<dyn TransportManager>,
    pub(super) publisher: Arc<ConfigPublisher>,
    pub(super) gates: StdMutex<HashMap<OAuthAccountId, Weak<Mutex<()>>>>,
    pub(super) refresh_state: OAuthRefreshState,
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
            refresh_state: OAuthRefreshState::new(),
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
        let active_versions = snapshot
            .oauth_accounts()
            .accounts()
            .iter()
            .map(|account| (account.id(), account.token_version()))
            .collect();
        self.retain_active_refresh_state(&active_versions);
        drop(snapshot);

        let mut prepared_segments = stream::iter(due)
            .map(|(id, token_version)| async move {
                (
                    id,
                    token_version,
                    self.prepare_scheduled_refresh(id, token_version).await,
                )
            })
            .buffer_unordered(SCHEDULED_REFRESH_CONCURRENCY)
            .ready_chunks(SCHEDULED_REFRESH_CONCURRENCY);
        let mut publications = ScanPublications::new(started_at);
        while let Some(prepared) = prepared_segments.next().await {
            if let Some(revision) = self.publish_prepared_refreshes(prepared).await {
                publications.record(revision);
            }
        }
        ScanOutcome {
            started_at,
            published: publications.latest,
            external_revision_interleaved: publications.external_revision_interleaved,
        }
    }

    async fn publish_prepared_refreshes(
        &self,
        prepared: Vec<(
            OAuthAccountId,
            u64,
            Result<Option<PreparedOAuthRefresh>, OAuthRefreshError>,
        )>,
    ) -> Option<ConfigRevision> {
        let mut refreshes = Vec::new();
        let mut pending = Vec::new();
        let mut gates = Vec::new();
        for (id, token_version, result) in prepared {
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
                Err(error) => {
                    if self.refresh_failure(id, token_version).is_none() {
                        self.record_refresh_failure(
                            id,
                            error.failure(token_version, OAuthRefreshTrigger::Scheduled),
                        );
                    }
                }
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
                        self.record_refresh_success(id, observed_token_version);
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
                let error = OAuthRefreshError::Publish(error);
                let mut first_failure = None;
                for (id, token_version) in pending.iter().copied() {
                    let failure = error.failure(token_version, OAuthRefreshTrigger::Scheduled);
                    first_failure.get_or_insert(failure);
                    self.record_refresh_failure(id, failure);
                }
                tracing::warn!(
                    prepared_account_count = pending.len(),
                    refresh_stage = ?first_failure.map(OAuthRefreshFailure::stage),
                    refresh_reason = ?first_failure.map(OAuthRefreshFailure::reason),
                    "OAuth account refresh batch publication failed"
                );
                None
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ScanOutcome {
    started_at: ConfigRevision,
    published: Option<ConfigRevision>,
    external_revision_interleaved: bool,
}

impl ScanOutcome {
    fn requires_immediate_rescan(self, latest: ConfigRevision) -> bool {
        self.external_revision_interleaved || latest > self.published.unwrap_or(self.started_at)
    }
}

struct ScanPublications {
    started_at: ConfigRevision,
    latest: Option<ConfigRevision>,
    external_revision_interleaved: bool,
}

impl ScanPublications {
    fn new(started_at: ConfigRevision) -> Self {
        Self {
            started_at,
            latest: None,
            external_revision_interleaved: false,
        }
    }

    fn record(&mut self, revision: ConfigRevision) {
        let previous = self.latest.unwrap_or(self.started_at);
        let expected = previous
            .checked_next()
            .expect("a successful publication proves the revision can advance");
        self.external_revision_interleaved |= revision != expected;
        self.latest = Some(revision);
    }
}

#[cfg(test)]
mod scan_outcome_tests {
    use super::{ScanOutcome, ScanPublications};
    use any2api_domain::ConfigRevision;

    #[test]
    fn contiguous_segment_revisions_are_all_recognized_as_this_scan() {
        let started_at = revision(10);
        let mut publications = ScanPublications::new(started_at);
        publications.record(revision(11));
        publications.record(revision(12));
        let outcome = ScanOutcome {
            started_at,
            published: publications.latest,
            external_revision_interleaved: publications.external_revision_interleaved,
        };

        assert!(!outcome.requires_immediate_rescan(revision(12)));
        assert!(outcome.requires_immediate_rescan(revision(13)));
    }

    #[test]
    fn a_revision_gap_between_segments_requires_a_rescan() {
        let started_at = revision(20);
        let mut publications = ScanPublications::new(started_at);
        publications.record(revision(21));
        publications.record(revision(23));
        let outcome = ScanOutcome {
            started_at,
            published: publications.latest,
            external_revision_interleaved: publications.external_revision_interleaved,
        };

        assert!(outcome.requires_immediate_rescan(revision(23)));
    }

    fn revision(value: u64) -> ConfigRevision {
        ConfigRevision::new(value).expect("test revision")
    }
}
