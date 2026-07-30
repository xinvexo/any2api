use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use reqwest::Client;
use semver::Version;

use crate::{
    api::{
        ApplicationAbout, ApplicationUpdateService, RestartRequester, UpdateCheck, UpdateError,
        UpdateErrorKind, UpdateStatus,
    },
    github::{self, REPOSITORY_URL},
    install,
    state::UpdateTaskState,
};

pub struct GitHubReleaseUpdater {
    inner: Arc<UpdaterInner>,
}

struct UpdaterInner {
    client: Client,
    current_version: Version,
    executable_path: PathBuf,
    install_support_reason: Option<String>,
    task: UpdateTaskState,
    restart: Arc<dyn RestartRequester>,
}

impl GitHubReleaseUpdater {
    pub(crate) fn new(
        executable_path: PathBuf,
        embedded_web: bool,
        restart: Arc<dyn RestartRequester>,
    ) -> Result<Self, UpdateError> {
        let current_version = Version::parse(crate::BUILD_VERSION).map_err(|error| {
            UpdateError::new(
                UpdateErrorKind::InvalidRelease,
                format!("current application version is invalid: {error}"),
            )
        })?;
        Ok(Self {
            inner: Arc::new(UpdaterInner {
                client: github::client()?,
                install_support_reason: install_support_reason(&executable_path, embedded_web),
                current_version,
                executable_path,
                task: UpdateTaskState::new(),
                restart,
            }),
        })
    }
}

impl UpdaterInner {
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

    async fn run_install(self: Arc<Self>) {
        let mut target_version = None;
        let result = self.install_latest(&mut target_version).await;
        if let Err(error) = result {
            let kind = error.kind();
            tracing::warn!(?kind, %error, "application update task failed");
            self.task.failed(target_version, kind);
        }
    }

    async fn install_latest(&self, target_version: &mut Option<String>) -> Result<(), UpdateError> {
        let (release, check) = self.latest_check().await?;
        let version = release.version.to_string();
        *target_version = Some(version.clone());
        if !check.update_available {
            return Err(UpdateError::new(
                UpdateErrorKind::NoUpdate,
                "the current version is already up to date",
            ));
        }

        self.task.downloading(&version, release.archive_size);
        let progress_task = &self.task;
        let installing_task = &self.task;
        install::replace_from_release(
            &self.client,
            &release,
            &self.executable_path,
            |downloaded_bytes| progress_task.downloaded(&version, downloaded_bytes),
            || installing_task.installing(&version),
        )
        .await?;
        self.task.restarting(&version);
        self.restart.request_restart();
        Ok(())
    }
}

#[async_trait]
impl ApplicationUpdateService for GitHubReleaseUpdater {
    fn about(&self) -> ApplicationAbout {
        ApplicationAbout {
            current_version: self.inner.current_version.to_string(),
            repository_url: REPOSITORY_URL.to_owned(),
        }
    }

    async fn check(&self) -> Result<UpdateCheck, UpdateError> {
        self.inner.latest_check().await.map(|(_, check)| check)
    }

    fn start_install(&self) -> Result<UpdateStatus, UpdateError> {
        if let Some(reason) = &self.inner.install_support_reason {
            return Err(UpdateError::new(
                UpdateErrorKind::Unsupported,
                reason.clone(),
            ));
        }
        let accepted = self.inner.task.begin()?;
        tokio::spawn(Arc::clone(&self.inner).run_install());
        Ok(accepted)
    }

    fn install_status(&self) -> UpdateStatus {
        self.inner.task.status()
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
