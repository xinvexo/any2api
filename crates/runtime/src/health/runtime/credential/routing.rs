use std::{
    collections::HashMap,
    mem,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{UpstreamErrorClassification, UpstreamErrorKind};
use tokio::time::Instant;

use super::super::{
    error::{HealthAcquireError, TemporaryUnavailability, TemporaryUnavailabilityCause},
    time::{deadline, max_deadline, retry_delay},
};
use crate::{
    health::ReliabilityPolicy,
    routing::{PendingSchedulerWakeNotification, SchedulerEpoch, SchedulerWakeSlot},
};

#[derive(Debug)]
pub(super) struct RoutingCredentialHealthRuntime {
    state: Mutex<RoutingCredentialHealthState>,
    scheduler_epoch: Arc<SchedulerEpoch>,
    credential_cooldown_wake: SchedulerWakeSlot,
    quota_exhaustion_wake: SchedulerWakeSlot,
}

#[derive(Debug, Default)]
struct RoutingCredentialHealthState {
    credential_cooldown_until: Option<Instant>,
    model_cooldowns: HashMap<String, ModelCooldown>,
    quota_exhaustion: Option<CredentialQuotaExhaustion>,
}

#[derive(Debug)]
struct ModelCooldown {
    until: Instant,
    cause: TemporaryUnavailabilityCause,
    wake: SchedulerWakeSlot,
}

impl ModelCooldown {
    const fn unavailability(&self) -> TemporaryUnavailability {
        TemporaryUnavailability::new(self.until, self.cause)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CredentialQuotaExhaustion {
    pub(crate) observed_at: i64,
    pub(crate) used: Option<u64>,
    pub(crate) limit: Option<u64>,
    retry_at: Instant,
}

impl RoutingCredentialHealthRuntime {
    pub(super) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        let credential_cooldown_wake = scheduler_epoch.wake_slot();
        let quota_exhaustion_wake = scheduler_epoch.wake_slot();
        Arc::new(Self {
            state: Mutex::new(RoutingCredentialHealthState::default()),
            scheduler_epoch,
            credential_cooldown_wake,
            quota_exhaustion_wake,
        })
    }

    pub(super) fn availability(&self, model: &str) -> Result<(), HealthAcquireError> {
        let now = Instant::now();
        let mut state = self.state.lock().expect("routing health lock poisoned");
        let expired_cooldowns = prune_expired_model_cooldowns(&mut state, now);
        let blocked = state
            .credential_cooldown_until
            .map(TemporaryUnavailability::outage)
            .into_iter()
            .chain(
                state
                    .model_cooldowns
                    .get(model)
                    .map(ModelCooldown::unavailability),
            )
            .chain(
                state
                    .quota_exhaustion
                    .map(|value| TemporaryUnavailability::rate_limit_cooldown(value.retry_at)),
            )
            .max_by_key(|unavailability| unavailability.until());
        let availability = match blocked {
            Some(unavailability) if now < unavailability.until() => {
                Err(HealthAcquireError::Temporary(unavailability))
            }
            _ => Ok(()),
        };
        drop(state);
        drop(expired_cooldowns);
        availability
    }

    pub(super) fn clear_temporary_cooldowns(&self) -> bool {
        let mut state = self.state.lock().expect("routing health lock poisoned");
        let had_credential_cooldown = state.credential_cooldown_until.take().is_some();
        let had_model_cooldown = !state.model_cooldowns.is_empty();
        let had_quota_exhaustion = state.quota_exhaustion.take().is_some();
        let model_cooldowns = mem::take(&mut state.model_cooldowns);
        let credential_wake_notification = self.credential_cooldown_wake.prepare_cancellation();
        let quota_wake_notification = self.quota_exhaustion_wake.prepare_cancellation();
        let changed = had_credential_cooldown || had_model_cooldown || had_quota_exhaustion;
        drop(state);
        credential_wake_notification.publish();
        quota_wake_notification.publish();
        drop(model_cooldowns);
        if changed {
            self.scheduler_epoch.advance();
        }
        changed
    }

    pub(super) fn quota_exhaustion(&self) -> Option<CredentialQuotaExhaustion> {
        self.state
            .lock()
            .expect("routing health lock poisoned")
            .quota_exhaustion
    }

    pub(super) fn clear_quota_exhaustion(&self) -> bool {
        let mut state = self.state.lock().expect("routing health lock poisoned");
        if state.quota_exhaustion.take().is_none() {
            return false;
        }
        let wake_notification = self.quota_exhaustion_wake.prepare_cancellation();
        drop(state);
        wake_notification.publish();
        self.scheduler_epoch.advance();
        true
    }

    pub(super) fn record_quota_exhaustion(
        &self,
        delay: Duration,
        used: Option<u64>,
        limit: Option<u64>,
    ) {
        let retry_at = deadline(Instant::now(), delay);
        let mut state = self.state.lock().expect("routing health lock poisoned");
        state.quota_exhaustion = Some(CredentialQuotaExhaustion {
            observed_at: unix_now(),
            used,
            limit,
            retry_at,
        });
        let wake_notification = self.quota_exhaustion_wake.prepare_schedule(retry_at);
        drop(state);
        wake_notification.publish();
        self.scheduler_epoch.advance();
    }

    pub(super) fn record_success(&self) {
        self.clear_quota_exhaustion();
    }

    pub(super) fn record(
        &self,
        model: &str,
        classification: UpstreamErrorClassification,
        policy: &ReliabilityPolicy,
    ) {
        let now = Instant::now();
        self.prune_expired_model_cooldowns(now);
        if classification.kind() == UpstreamErrorKind::QuotaExhausted {
            let numeric = classification.quota_exhaustion();
            self.record_quota_exhaustion(
                retry_delay(classification.retry_after(), policy.permission_denied),
                numeric.map(|value| value.used()),
                numeric.map(|value| value.limit()),
            );
            return;
        }
        let mut state = self.state.lock().expect("routing health lock poisoned");
        let wake_notification = match classification.kind() {
            UpstreamErrorKind::PermissionDenied => {
                let until = deadline(now, policy.permission_denied);
                state.credential_cooldown_until =
                    max_deadline(state.credential_cooldown_until, Some(until));
                Some(
                    self.credential_cooldown_wake.prepare_schedule(
                        state
                            .credential_cooldown_until
                            .expect("permission cooldown was just recorded"),
                    ),
                )
            }
            UpstreamErrorKind::RateLimited => {
                let delay = retry_delay(classification.retry_after(), policy.rate_limit_fallback);
                Some(record_model_cooldown(
                    &mut state,
                    model,
                    deadline(now, delay),
                    TemporaryUnavailabilityCause::RateLimitCooldown,
                    &self.scheduler_epoch,
                ))
            }
            UpstreamErrorKind::ModelUnavailable => Some(record_model_cooldown(
                &mut state,
                model,
                deadline(now, policy.model_unsupported),
                TemporaryUnavailabilityCause::Outage,
                &self.scheduler_epoch,
            )),
            _ => None,
        };
        drop(state);
        if let Some(wake_notification) = wake_notification {
            wake_notification.publish();
        }
    }

    fn prune_expired_model_cooldowns(&self, now: Instant) {
        let mut state = self.state.lock().expect("routing health lock poisoned");
        let expired_cooldowns = prune_expired_model_cooldowns(&mut state, now);
        drop(state);
        drop(expired_cooldowns);
    }

    #[cfg(test)]
    pub(super) fn model_cooldown_count(&self) -> usize {
        self.state
            .lock()
            .expect("routing health lock poisoned")
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
    state: &mut RoutingCredentialHealthState,
    model: &str,
    until: Instant,
    cause: TemporaryUnavailabilityCause,
    scheduler_epoch: &Arc<SchedulerEpoch>,
) -> PendingSchedulerWakeNotification {
    let entry = state
        .model_cooldowns
        .entry(model.to_owned())
        .or_insert_with(|| ModelCooldown {
            until,
            cause,
            wake: scheduler_epoch.wake_slot(),
        });
    if until >= entry.until {
        entry.until = until;
        entry.cause = cause;
    }
    entry.wake.prepare_schedule(entry.until)
}

fn prune_expired_model_cooldowns(
    state: &mut RoutingCredentialHealthState,
    now: Instant,
) -> Vec<ModelCooldown> {
    let expired_models = state
        .model_cooldowns
        .iter()
        .filter(|(_, cooldown)| cooldown.until <= now)
        .map(|(model, _)| model.clone())
        .collect::<Vec<_>>();
    expired_models
        .into_iter()
        .filter_map(|model| state.model_cooldowns.remove(&model))
        .collect()
}
