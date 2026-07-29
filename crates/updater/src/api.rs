use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;

pub use crate::service::GitHubReleaseUpdater;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationAbout {
    pub current_version: String,
    pub repository_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateCheck {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateInstall {
    pub installed_version: String,
    pub restart_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateErrorKind {
    Unsupported,
    CheckFailed,
    InvalidRelease,
    NoUpdate,
    InProgress,
    DownloadFailed,
    VerificationFailed,
    InstallFailed,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct UpdateError {
    kind: UpdateErrorKind,
    message: String,
}

impl UpdateError {
    #[must_use]
    pub fn new(kind: UpdateErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> UpdateErrorKind {
        self.kind
    }
}

pub trait RestartRequester: Send + Sync {
    fn request_restart(&self);
}

#[async_trait]
pub trait ApplicationUpdateService: Send + Sync {
    fn about(&self) -> ApplicationAbout;

    async fn check(&self) -> Result<UpdateCheck, UpdateError>;

    async fn install(&self) -> Result<UpdateInstall, UpdateError>;
}

impl GitHubReleaseUpdater {
    pub fn official(
        executable_path: PathBuf,
        embedded_web: bool,
        restart: Arc<dyn RestartRequester>,
    ) -> Result<Self, UpdateError> {
        Self::new(executable_path, embedded_web, restart)
    }
}
