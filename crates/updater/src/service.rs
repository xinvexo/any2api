use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use reqwest::Client;
use semver::Version;
use tokio::sync::Mutex;

use crate::{
    api::{
        ApplicationAbout, ApplicationUpdateService, RestartRequester, UpdateCheck, UpdateError,
        UpdateErrorKind, UpdateInstall,
    },
    github::{self, REPOSITORY_URL},
    install,
};

pub struct GitHubReleaseUpdater {
    client: Client,
    current_version: Version,
    executable_path: PathBuf,
    install_support_reason: Option<String>,
    install_gate: Mutex<()>,
    restart: Arc<dyn RestartRequester>,
}

impl GitHubReleaseUpdater {
    pub(crate) fn new(
        executable_path: PathBuf,
        embedded_web: bool,
        restart: Arc<dyn RestartRequester>,
    ) -> Result<Self, UpdateError> {
        let current_version = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|error| {
            UpdateError::new(
                UpdateErrorKind::InvalidRelease,
                format!("current application version is invalid: {error}"),
            )
        })?;
        Ok(Self {
            client: github::client()?,
            install_support_reason: install_support_reason(&executable_path, embedded_web),
            current_version,
            executable_path,
            install_gate: Mutex::new(()),
            restart,
        })
    }

    async fn latest_check(&self) -> Result<(github::Release, UpdateCheck), UpdateError> {
        let release = github::latest(&self.client).await?;
        let check = UpdateCheck {
            current_version: self.current_version.to_string(),
            latest_version: release.version.to_string(),
            update_available: release.version > self.current_version,
            release_url: release.release_url.clone(),
            published_at: release.published_at.clone(),
        };
        Ok((release, check))
    }
}

#[async_trait]
impl ApplicationUpdateService for GitHubReleaseUpdater {
    fn about(&self) -> ApplicationAbout {
        ApplicationAbout {
            current_version: self.current_version.to_string(),
            repository_url: REPOSITORY_URL.to_owned(),
        }
    }

    async fn check(&self) -> Result<UpdateCheck, UpdateError> {
        self.latest_check().await.map(|(_, check)| check)
    }

    async fn install(&self) -> Result<UpdateInstall, UpdateError> {
        let _gate = self.install_gate.try_lock().map_err(|_| {
            UpdateError::new(
                UpdateErrorKind::InProgress,
                "an update is already in progress",
            )
        })?;
        if let Some(reason) = &self.install_support_reason {
            return Err(UpdateError::new(
                UpdateErrorKind::Unsupported,
                reason.clone(),
            ));
        }
        let (release, check) = self.latest_check().await?;
        if !check.update_available {
            return Err(UpdateError::new(
                UpdateErrorKind::NoUpdate,
                "the current version is already up to date",
            ));
        }
        install::replace_from_release(&self.client, &release, &self.executable_path).await?;
        self.restart.request_restart();
        Ok(UpdateInstall {
            installed_version: release.version.to_string(),
            restart_requested: true,
        })
    }
}

fn install_support_reason(executable_path: &Path, embedded_web: bool) -> Option<String> {
    if !cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu"
    )) {
        return Some("in-place updates require Linux AMD64 GNU".to_owned());
    }
    if cfg!(debug_assertions) {
        return Some("in-place updates are disabled for development builds".to_owned());
    }
    if !embedded_web {
        return Some("in-place updates are disabled while ANY2API_WEB_DIR is set".to_owned());
    }
    if !executable_path.is_file() || executable_path.parent().is_none() {
        return Some("the current executable cannot be replaced in place".to_owned());
    }
    None
}
