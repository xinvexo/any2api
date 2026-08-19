use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;

pub use crate::recovery::{StartupUpdateRecovery, UpdateRecoveryError, recover_pending_update};
pub use crate::service::GitHubReleaseUpdater;

pub const APPLICATION_VERSION: &str = crate::BUILD_VERSION;

pub type UpdateTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
pub type UpdateCommitTask = Box<dyn FnOnce() + Send + 'static>;
pub type UpdateBlockingTask = Box<dyn FnOnce() -> Result<(), UpdateError> + Send + 'static>;
pub type UpdateBlockingFuture =
    Pin<Box<dyn Future<Output = Result<(), UpdateError>> + Send + 'static>>;

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
pub enum UpdateStatus {
    Idle,
    Checking,
    Downloading {
        target_version: String,
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    Installing {
        target_version: String,
    },
    Restarting {
        target_version: String,
    },
    Failed {
        target_version: Option<String>,
        kind: UpdateErrorKind,
    },
}

impl UpdateStatus {
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(
            self,
            Self::Checking
                | Self::Downloading { .. }
                | Self::Installing { .. }
                | Self::Restarting { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateErrorKind {
    Unsupported,
    CheckFailed,
    InvalidRelease,
    NoUpdate,
    InProgress,
    ShuttingDown,
    DownloadFailed,
    VerificationFailed,
    InstallFailed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RestartKind {
    Manual = 1,
    Update = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestartRequestStatus {
    Accepted,
    AlreadyRequested,
    Unsupported,
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
    fn request_restart(&self, kind: RestartKind) -> RestartRequestStatus;
}

pub trait UpdateTaskExecutor: Send + Sync {
    fn accepts_new_tasks(&self) -> bool;

    /// A `false` result guarantees that `task` was never polled.
    fn try_spawn(&self, task: UpdateTask) -> bool;

    /// Runs blocking preparation work under process-lifecycle tracking.
    ///
    /// Once the returned future has started the task, dropping that future may
    /// stop observing its result but must not remove the task from lifecycle
    /// tracking before the blocking work itself finishes.
    fn run_blocking(&self, task: UpdateBlockingTask) -> UpdateBlockingFuture;

    /// Registers an accepted update's commit on a tracked blocking executor.
    ///
    /// Once this method returns, `task` must run to completion even if the
    /// asynchronous update task is cancelled.
    fn spawn_blocking_commit(&self, task: UpdateCommitTask);
}

#[async_trait]
pub trait ApplicationUpdateService: Send + Sync {
    fn about(&self) -> ApplicationAbout;

    async fn check(&self) -> Result<UpdateCheck, UpdateError>;

    fn start_install(&self) -> Result<UpdateStatus, UpdateError>;

    fn install_status(&self) -> UpdateStatus;
}

impl GitHubReleaseUpdater {
    /// Builds the production updater; stale-workspace cleanup runs on a
    /// blocking thread to keep synchronous directory IO off the async workers.
    pub async fn official(
        executable_path: PathBuf,
        embedded_web: bool,
        restart: Arc<dyn RestartRequester>,
        tasks: Arc<dyn UpdateTaskExecutor>,
    ) -> Result<Self, UpdateError> {
        let cleanup_target = executable_path.clone();
        let cleanup = tasks
            .run_blocking(Box::new(move || {
                Self::cleanup_stale_workspaces(&cleanup_target);
                Ok(())
            }))
            .await;
        if let Err(error) = cleanup {
            tracing::warn!(%error, "stale update cleanup task failed during initialization");
        }
        Self::new(executable_path, embedded_web, restart, tasks)
    }
}
