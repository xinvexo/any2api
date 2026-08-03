use std::sync::{Arc, Mutex};

use super::super::error::HealthAcquireError;
use crate::routing::SchedulerEpoch;

#[derive(Debug)]
pub(super) struct AuthenticationHealthRuntime {
    auth_error: Mutex<bool>,
    scheduler_epoch: Arc<SchedulerEpoch>,
}

impl AuthenticationHealthRuntime {
    pub(super) fn new(scheduler_epoch: Arc<SchedulerEpoch>) -> Self {
        Self {
            auth_error: Mutex::new(false),
            scheduler_epoch,
        }
    }

    pub(super) fn availability(&self) -> Result<(), HealthAcquireError> {
        if *self
            .auth_error
            .lock()
            .expect("authentication health lock poisoned")
        {
            Err(HealthAcquireError::Permanent)
        } else {
            Ok(())
        }
    }

    pub(super) fn clear_auth_error(&self) -> bool {
        self.update(false)
    }

    pub(super) fn record_authentication_failure(&self) -> bool {
        self.update(true)
    }

    #[cfg(test)]
    pub(super) fn has_auth_error(&self) -> bool {
        *self
            .auth_error
            .lock()
            .expect("authentication health lock poisoned")
    }

    fn update(&self, value: bool) -> bool {
        let mut auth_error = self
            .auth_error
            .lock()
            .expect("authentication health lock poisoned");
        if *auth_error == value {
            return false;
        }
        *auth_error = value;
        drop(auth_error);
        self.scheduler_epoch.advance();
        true
    }
}
