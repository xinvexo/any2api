use std::{collections::HashMap, sync::Mutex};

use any2api_domain::OAuthAccountId;
use any2api_provider::api::OAuthRefreshRejection;
use tokio::sync::watch;

use super::{failure::OAuthRefreshFailure, worker::OAuthRefresher};

pub(super) struct OAuthRefreshState {
    permanent_rejections: Mutex<HashMap<OAuthAccountId, PermanentRefreshRejection>>,
    failed_attempts: Mutex<HashMap<OAuthAccountId, FailedRefreshAttempts>>,
    failures: Mutex<HashMap<OAuthAccountId, OAuthRefreshFailure>>,
    failure_changes: watch::Sender<u64>,
}

#[derive(Clone, Copy, Debug)]
struct FailedRefreshAttempts {
    token_version: u64,
    attempts: u64,
}

#[derive(Clone, Copy, Debug)]
struct PermanentRefreshRejection {
    token_version: u64,
    rejection: OAuthRefreshRejection,
}

impl OAuthRefreshState {
    pub(super) fn new() -> Self {
        let (failure_changes, _) = watch::channel(0);
        Self {
            permanent_rejections: Mutex::new(HashMap::new()),
            failed_attempts: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
            failure_changes,
        }
    }
}

impl OAuthRefresher {
    pub(super) fn permanent_rejection(
        &self,
        id: OAuthAccountId,
        token_version: u64,
    ) -> Option<OAuthRefreshRejection> {
        let mut rejected = self
            .refresh_state
            .permanent_rejections
            .lock()
            .expect("OAuth refresh rejection lock poisoned");
        match rejected.get(&id).copied() {
            Some(recorded) if recorded.token_version == token_version => Some(recorded.rejection),
            Some(_) => {
                rejected.remove(&id);
                None
            }
            None => None,
        }
    }

    pub(super) fn record_permanent_rejection(
        &self,
        id: OAuthAccountId,
        token_version: u64,
        rejection: OAuthRefreshRejection,
    ) {
        self.refresh_state
            .permanent_rejections
            .lock()
            .expect("OAuth refresh rejection lock poisoned")
            .insert(
                id,
                PermanentRefreshRejection {
                    token_version,
                    rejection,
                },
            );
    }

    pub(crate) fn refresh_failure(
        &self,
        id: OAuthAccountId,
        token_version: u64,
    ) -> Option<OAuthRefreshFailure> {
        self.refresh_state
            .failures
            .lock()
            .expect("OAuth refresh failure lock poisoned")
            .get(&id)
            .copied()
            .filter(|failure| failure.token_version() == token_version)
    }

    pub(crate) fn subscribe_refresh_failure_changes(&self) -> watch::Receiver<u64> {
        self.refresh_state.failure_changes.subscribe()
    }

    pub(super) fn record_refresh_failure(&self, id: OAuthAccountId, failure: OAuthRefreshFailure) {
        let changed = self
            .refresh_state
            .failures
            .lock()
            .expect("OAuth refresh failure lock poisoned")
            .insert(id, failure)
            != Some(failure);
        if changed {
            tracing::warn!(
                event = "oauth_token_refresh_failed",
                oauth_account_id = %id,
                token_version = failure.token_version(),
                refresh_trigger = ?failure.trigger(),
                refresh_stage = ?failure.stage(),
                refresh_reason = ?failure.reason(),
                upstream_status = failure.upstream_status(),
                failure_scope = ?failure.failure_scope(),
                reauthorization_required = failure.reauthorization_required(),
                occurred_at = failure.occurred_at(),
                "OAuth account token refresh failed"
            );
            self.notify_refresh_failure_changed();
        }
    }

    pub(crate) fn record_refreshed_access_token_rejected(
        &self,
        id: OAuthAccountId,
        token_version: u64,
    ) -> OAuthRefreshFailure {
        let failure = OAuthRefreshFailure::refreshed_access_token_rejected(token_version);
        self.record_refresh_failure(id, failure);
        failure
    }

    pub(super) fn record_refresh_success(&self, id: OAuthAccountId, observed_token_version: u64) {
        let changed = {
            let mut failures = self
                .refresh_state
                .failures
                .lock()
                .expect("OAuth refresh failure lock poisoned");
            failures
                .get(&id)
                .is_some_and(|failure| failure.token_version() <= observed_token_version)
                && failures.remove(&id).is_some()
        };
        self.refresh_state
            .permanent_rejections
            .lock()
            .expect("OAuth refresh rejection lock poisoned")
            .remove(&id);
        self.refresh_state
            .failed_attempts
            .lock()
            .expect("OAuth refresh attempt lock poisoned")
            .remove(&id);
        if changed {
            self.notify_refresh_failure_changed();
        }
    }

    pub(super) fn failed_refresh_attempts(&self, id: OAuthAccountId, token_version: u64) -> u64 {
        self.refresh_state
            .failed_attempts
            .lock()
            .expect("OAuth refresh attempt lock poisoned")
            .get(&id)
            .filter(|failed| failed.token_version == token_version)
            .map_or(0, |failed| failed.attempts)
    }

    pub(super) fn record_failed_refresh_attempt(&self, id: OAuthAccountId, token_version: u64) {
        let mut attempts = self
            .refresh_state
            .failed_attempts
            .lock()
            .expect("OAuth refresh attempt lock poisoned");
        let failed = attempts.entry(id).or_insert(FailedRefreshAttempts {
            token_version,
            attempts: 0,
        });
        if failed.token_version != token_version {
            *failed = FailedRefreshAttempts {
                token_version,
                attempts: 0,
            };
        }
        failed.attempts += 1;
    }

    pub(super) fn retain_active_refresh_state(&self, active: &HashMap<OAuthAccountId, u64>) {
        self.gates
            .lock()
            .expect("OAuth refresh gate lock poisoned")
            .retain(|id, gate| active.contains_key(id) || gate.strong_count() > 0);
        self.refresh_state
            .permanent_rejections
            .lock()
            .expect("OAuth refresh rejection lock poisoned")
            .retain(|id, rejected| active.get(id) == Some(&rejected.token_version));
        self.refresh_state
            .failed_attempts
            .lock()
            .expect("OAuth refresh attempt lock poisoned")
            .retain(|id, failed| active.get(id) == Some(&failed.token_version));
        let failures_changed = {
            let mut failures = self
                .refresh_state
                .failures
                .lock()
                .expect("OAuth refresh failure lock poisoned");
            let before = failures.len();
            failures.retain(|id, failure| active.get(id) == Some(&failure.token_version()));
            failures.len() != before
        };
        if failures_changed {
            self.notify_refresh_failure_changed();
        }
    }

    fn notify_refresh_failure_changed(&self) {
        let next = (*self.refresh_state.failure_changes.borrow()).wrapping_add(1);
        self.refresh_state.failure_changes.send_replace(next);
    }
}
