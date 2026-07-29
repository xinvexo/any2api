use any2api_updater::api::{ApplicationAbout, UpdateCheck, UpdateInstall};
use serde::Serialize;

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
pub(crate) struct UpdateInstallResponse {
    installed_version: String,
    restart_requested: bool,
}

impl From<UpdateInstall> for UpdateInstallResponse {
    fn from(value: UpdateInstall) -> Self {
        Self {
            installed_version: value.installed_version,
            restart_requested: value.restart_requested,
        }
    }
}
