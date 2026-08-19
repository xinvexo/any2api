use any2api_updater::api::{ApplicationAbout, UpdateCheck, UpdateStatus};
use serde::Serialize;

use super::error::stable_error_code;

#[derive(Debug, Serialize)]
pub(crate) struct RestartResponse {
    status: &'static str,
}

impl RestartResponse {
    pub(crate) const fn restarting() -> Self {
        Self {
            status: "restarting",
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AboutResponse {
    current_version: String,
    repository_url: String,
}

impl From<ApplicationAbout> for AboutResponse {
    fn from(value: ApplicationAbout) -> Self {
        Self {
            current_version: value.current_version,
            repository_url: value.repository_url,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateCheckResponse {
    current_version: String,
    latest_version: String,
    update_available: bool,
    release_url: String,
    published_at: Option<String>,
}

impl From<UpdateCheck> for UpdateCheckResponse {
    fn from(value: UpdateCheck) -> Self {
        Self {
            current_version: value.current_version,
            latest_version: value.latest_version,
            update_available: value.update_available,
            release_url: value.release_url,
            published_at: value.published_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateStatusResponse {
    phase: &'static str,
    target_version: Option<String>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    failure_code: Option<&'static str>,
}

impl From<UpdateStatus> for UpdateStatusResponse {
    fn from(value: UpdateStatus) -> Self {
        match value {
            UpdateStatus::Idle => Self::simple("idle", None),
            UpdateStatus::Checking => Self::simple("checking", None),
            UpdateStatus::Downloading {
                target_version,
                downloaded_bytes,
                total_bytes,
            } => Self {
                phase: "downloading",
                target_version: Some(target_version),
                downloaded_bytes: Some(downloaded_bytes),
                total_bytes: Some(total_bytes),
                failure_code: None,
            },
            UpdateStatus::Installing { target_version } => {
                Self::simple("installing", Some(target_version))
            }
            UpdateStatus::Restarting { target_version } => {
                Self::simple("restarting", Some(target_version))
            }
            UpdateStatus::Failed {
                target_version,
                kind,
            } => Self {
                failure_code: Some(stable_error_code(kind)),
                ..Self::simple("failed", target_version)
            },
        }
    }
}

impl UpdateStatusResponse {
    fn simple(phase: &'static str, target_version: Option<String>) -> Self {
        Self {
            phase,
            target_version,
            downloaded_bytes: None,
            total_bytes: None,
            failure_code: None,
        }
    }
}
