use std::sync::{Mutex, MutexGuard};

use crate::api::{UpdateError, UpdateErrorKind, UpdateStatus};

pub(crate) struct UpdateTaskState {
    status: Mutex<UpdateStatus>,
}

impl UpdateTaskState {
    pub(crate) fn new() -> Self {
        Self {
            status: Mutex::new(UpdateStatus::Idle),
        }
    }

    pub(crate) fn begin(&self) -> Result<UpdateStatus, UpdateError> {
        let mut status = self.lock();
        if status.is_active() {
            return Err(UpdateError::new(
                UpdateErrorKind::InProgress,
                "an update is already in progress",
            ));
        }
        *status = UpdateStatus::Checking;
        Ok(status.clone())
    }

    pub(crate) fn status(&self) -> UpdateStatus {
        self.lock().clone()
    }

    pub(crate) fn downloading(&self, target_version: &str, total_bytes: u64) {
        *self.lock() = UpdateStatus::Downloading {
            target_version: target_version.to_owned(),
            downloaded_bytes: 0,
            total_bytes,
        };
    }

    pub(crate) fn downloaded(&self, target_version: &str, downloaded_bytes: u64) {
        let mut status = self.lock();
        let UpdateStatus::Downloading {
            target_version: current_target,
            downloaded_bytes: current_bytes,
            total_bytes,
        } = &mut *status
        else {
            return;
        };
        if current_target == target_version {
            *current_bytes = downloaded_bytes.min(*total_bytes).max(*current_bytes);
        }
    }

    pub(crate) fn installing(&self, target_version: &str) {
        *self.lock() = UpdateStatus::Installing {
            target_version: target_version.to_owned(),
        };
    }

    pub(crate) fn restarting(&self, target_version: &str) {
        *self.lock() = UpdateStatus::Restarting {
            target_version: target_version.to_owned(),
        };
    }

    pub(crate) fn failed(&self, target_version: Option<String>, kind: UpdateErrorKind) {
        *self.lock() = UpdateStatus::Failed {
            target_version,
            kind,
        };
    }

    fn lock(&self) -> MutexGuard<'_, UpdateStatus> {
        self.status
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_active_task_is_admitted_and_failure_allows_retry() {
        let state = UpdateTaskState::new();
        assert_eq!(state.begin().expect("first task"), UpdateStatus::Checking);
        assert_eq!(
            state.begin().expect_err("second task").kind(),
            UpdateErrorKind::InProgress
        );

        state.failed(None, UpdateErrorKind::CheckFailed);
        assert_eq!(state.begin().expect("retry"), UpdateStatus::Checking);
    }

    #[test]
    fn download_progress_is_monotonic_and_bounded() {
        let state = UpdateTaskState::new();
        state.downloading("1.2.3", 100);
        state.downloaded("1.2.3", 60);
        state.downloaded("1.2.3", 40);
        state.downloaded("1.2.3", 120);

        assert_eq!(
            state.status(),
            UpdateStatus::Downloading {
                target_version: "1.2.3".to_owned(),
                downloaded_bytes: 100,
                total_bytes: 100,
            }
        );
    }
}
