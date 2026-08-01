use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use any2api_domain::{UpstreamErrorClassification, UpstreamErrorKind};
use tokio::time::Instant;

use super::{
    error::HealthAcquireError,
    time::{deadline, max_deadline, retry_delay},
};
use crate::{
    health::ReliabilityPolicy,
    routing::{SchedulerEpoch, SchedulerWakeSlot},
};

#[derive(Debug)]
pub(crate) struct CredentialHealthRuntime {
    state: Mutex<CredentialHealthState>,
    scheduler_epoch: Arc<SchedulerEpoch>,
    credential_cooldown_wake: SchedulerWakeSlot,
    quota_exhaustion_wake: SchedulerWakeSlot,
}

#[derive(Debug, Default)]
struct CredentialHealthState {
    auth_error: bool,
    credential_cooldown_until: Option<Instant>,
    model_cooldowns: HashMap<String, ModelCooldown>,
    quota_exhaustion: Option<CredentialQuotaExhaustion>,
}

#[derive(Debug)]
struct ModelCooldown {
    until: Instant,
    wake: SchedulerWakeSlot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CredentialQuotaExhaustion {
    pub(crate) observed_at: i64,
    pub(crate) used: Option<u64>,
    pub(crate) limit: Option<u64>,
    retry_at: Instant,
}

impl CredentialHealthRuntime {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        let credential_cooldown_wake = scheduler_epoch.wake_slot();
        let quota_exhaustion_wake = scheduler_epoch.wake_slot();
        Arc::new(Self {
            state: Mutex::new(CredentialHealthState::default()),
            scheduler_epoch,
            credential_cooldown_wake,
            quota_exhaustion_wake,
        })
    }

    pub(crate) fn availability(&self, model: &str) -> Result<(), HealthAcquireError> {
        let now = Instant::now();
        let mut state = self.state.lock().expect("credential health lock poisoned");
        prune_expired_model_cooldowns(&mut state, now);
        if state.auth_error {
            return Err(HealthAcquireError::Permanent);
        }
        let until = state
            .credential_cooldown_until
            .into_iter()
            .chain(
                state
                    .model_cooldowns
                    .get(model)
                    .map(|cooldown| cooldown.until),
            )
            .chain(state.quota_exhaustion.map(|value| value.retry_at))
            .max();
        match until {
            Some(until) if now < until => Err(HealthAcquireError::Temporary(until)),
            _ => Ok(()),
        }
    }

    pub(crate) fn clear_auth_error(&self) -> bool {
        let mut state = self.state.lock().expect("credential health lock poisoned");
        if !state.auth_error {
            return false;
        }
        state.auth_error = false;
        drop(state);
        self.scheduler_epoch.advance();
        true
    }

    pub(crate) fn record_authentication_failure(&self) -> bool {
        let mut state = self.state.lock().expect("credential health lock poisoned");
        if state.auth_error {
            return false;
        }
        state.auth_error = true;
        drop(state);
        self.scheduler_epoch.advance();
        true
    }

    pub(crate) fn clear_temporary_cooldowns(&self) -> bool {
        let mut state = self.state.lock().expect("credential health lock poisoned");
        let had_credential_cooldown = state.credential_cooldown_until.take().is_some();
        let had_model_cooldown = !state.model_cooldowns.is_empty();
        let had_quota_exhaustion = state.quota_exhaustion.take().is_some();
        state.model_cooldowns.clear();
        self.credential_cooldown_wake.cancel();
        self.quota_exhaustion_wake.cancel();
        let changed = had_credential_cooldown || had_model_cooldown || had_quota_exhaustion;
        drop(state);
        if changed {
            self.scheduler_epoch.advance();
        }
        changed
    }

    #[cfg(test)]
    pub(crate) fn has_auth_error(&self) -> bool {
        self.state
            .lock()
            .expect("credential health lock poisoned")
            .auth_error
    }

    pub(crate) fn quota_exhaustion(&self) -> Option<CredentialQuotaExhaustion> {
        self.state
            .lock()
            .expect("credential health lock poisoned")
            .quota_exhaustion
    }

    pub(crate) fn clear_quota_exhaustion(&self) -> bool {
        let mut state = self.state.lock().expect("credential health lock poisoned");
        if state.quota_exhaustion.take().is_none() {
            return false;
        }
        self.quota_exhaustion_wake.cancel();
        drop(state);
        self.scheduler_epoch.advance();
        true
    }

    pub(crate) fn record_quota_exhaustion(
        &self,
        delay: std::time::Duration,
        used: Option<u64>,
        limit: Option<u64>,
    ) {
        let retry_at = deadline(Instant::now(), delay);
        let mut state = self.state.lock().expect("credential health lock poisoned");
        state.quota_exhaustion = Some(CredentialQuotaExhaustion {
            observed_at: unix_now(),
            used,
            limit,
            retry_at,
        });
        self.quota_exhaustion_wake.schedule(retry_at);
        drop(state);
        self.scheduler_epoch.advance();
    }

    pub(crate) fn record_success(&self) {
        self.clear_quota_exhaustion();
    }

    pub(crate) fn record(
        &self,
        model: &str,
        classification: UpstreamErrorClassification,
        policy: &ReliabilityPolicy,
    ) {
        let now = Instant::now();
        self.prune_expired_model_cooldowns(now);
        if classification.kind() == UpstreamErrorKind::Authentication {
            self.record_authentication_failure();
            return;
        }
        if classification.kind() == UpstreamErrorKind::QuotaExhausted {
            let numeric = classification.quota_exhaustion();
            self.record_quota_exhaustion(
                retry_delay(classification.retry_after(), policy.permission_denied),
                numeric.map(|value| value.used()),
                numeric.map(|value| value.limit()),
            );
            return;
        }
        let mut state = self.state.lock().expect("credential health lock poisoned");
        match classification.kind() {
            UpstreamErrorKind::PermissionDenied => {
                let until = deadline(now, policy.permission_denied);
                state.credential_cooldown_until =
                    max_deadline(state.credential_cooldown_until, Some(until));
                self.credential_cooldown_wake.schedule(
                    state
                        .credential_cooldown_until
                        .expect("permission cooldown was just recorded"),
                );
            }
            UpstreamErrorKind::RateLimited => {
                let delay = retry_delay(classification.retry_after(), policy.rate_limit_fallback);
                record_model_cooldown(
                    &mut state,
                    model,
                    deadline(now, delay),
                    &self.scheduler_epoch,
                );
            }
            UpstreamErrorKind::ModelUnavailable => record_model_cooldown(
                &mut state,
                model,
                deadline(now, policy.model_unsupported),
                &self.scheduler_epoch,
            ),
            _ => {}
        }
    }

    fn prune_expired_model_cooldowns(&self, now: Instant) {
        let mut state = self.state.lock().expect("credential health lock poisoned");
        prune_expired_model_cooldowns(&mut state, now);
    }

    #[cfg(test)]
    pub(crate) fn model_cooldown_count(&self) -> usize {
        self.state
            .lock()
            .expect("credential health lock poisoned")
            .model_cooldowns
            .len()
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

fn record_model_cooldown(
    state: &mut CredentialHealthState,
    model: &str,
    until: Instant,
    scheduler_epoch: &Arc<SchedulerEpoch>,
) {
    let entry = state
        .model_cooldowns
        .entry(model.to_owned())
        .or_insert_with(|| ModelCooldown {
            until,
            wake: scheduler_epoch.wake_slot(),
        });
    entry.until = entry.until.max(until);
    entry.wake.schedule(entry.until);
}

fn prune_expired_model_cooldowns(state: &mut CredentialHealthState, now: Instant) {
    state
        .model_cooldowns
        .retain(|_, cooldown| cooldown.until > now);
}
