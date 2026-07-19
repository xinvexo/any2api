use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use any2api_domain::{UpstreamErrorClassification, UpstreamErrorKind};
use tokio::time::Instant;

use super::{
    error::HealthAcquireError,
    time::{deadline, max_deadline, retry_delay, schedule_wake},
};
use crate::{health::ReliabilityPolicy, scheduler_epoch::SchedulerEpoch};

#[derive(Debug)]
pub(crate) struct CredentialHealthRuntime {
    state: Mutex<CredentialHealthState>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

#[derive(Debug, Default)]
struct CredentialHealthState {
    auth_error: bool,
    credential_cooldown_until: Option<Instant>,
    model_cooldowns: HashMap<String, Instant>,
}

impl CredentialHealthRuntime {
    pub(crate) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(CredentialHealthState::default()),
            scheduler_epoch,
        })
    }

    pub(crate) fn availability(&self, model: &str) -> Result<(), HealthAcquireError> {
        let now = Instant::now();
        let state = self.state.lock().expect("credential health lock poisoned");
        if state.auth_error {
            return Err(HealthAcquireError::Permanent);
        }
        let until = state
            .credential_cooldown_until
            .into_iter()
            .chain(state.model_cooldowns.get(model).copied())
            .max();
        match until {
            Some(until) if now < until => Err(HealthAcquireError::Temporary(until)),
            _ => Ok(()),
        }
    }

    pub(crate) fn record(
        &self,
        model: &str,
        classification: UpstreamErrorClassification,
        policy: &ReliabilityPolicy,
    ) {
        let now = Instant::now();
        let mut state = self.state.lock().expect("credential health lock poisoned");
        let wake_at = match classification.kind() {
            UpstreamErrorKind::Authentication => {
                state.auth_error = true;
                None
            }
            UpstreamErrorKind::PermissionDenied | UpstreamErrorKind::QuotaExhausted => {
                let until = deadline(now, policy.permission_denied);
                state.credential_cooldown_until =
                    max_deadline(state.credential_cooldown_until, Some(until));
                Some(until)
            }
            UpstreamErrorKind::RateLimited => {
                let delay = retry_delay(classification.retry_after(), policy.rate_limit_fallback);
                Some(record_model_cooldown(
                    &mut state,
                    model,
                    deadline(now, delay),
                ))
            }
            UpstreamErrorKind::ModelUnavailable => Some(record_model_cooldown(
                &mut state,
                model,
                deadline(now, policy.model_unsupported),
            )),
            _ => None,
        };
        drop(state);
        if let Some(wake_at) = wake_at {
            schedule_wake(Arc::clone(&self.scheduler_epoch), wake_at);
        }
    }
}

fn record_model_cooldown(
    state: &mut CredentialHealthState,
    model: &str,
    until: Instant,
) -> Instant {
    let entry = state
        .model_cooldowns
        .entry(model.to_owned())
        .or_insert(until);
    *entry = (*entry).max(until);
    *entry
}
