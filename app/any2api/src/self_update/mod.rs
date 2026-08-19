use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use any2api_runtime::api::{ProcessLifecycle, ShutdownPhase};
use any2api_updater::api::{
    RestartKind, RestartRequestStatus, RestartRequester, UpdateBlockingFuture, UpdateBlockingTask,
    UpdateCommitTask, UpdateError, UpdateErrorKind, UpdateTask, UpdateTaskExecutor,
};
use tokio_util::sync::CancellationToken;

const NO_RESTART: u8 = 0;

#[derive(Clone)]
pub(crate) struct LifecycleUpdateTaskExecutor {
    lifecycle: ProcessLifecycle,
}

impl LifecycleUpdateTaskExecutor {
    pub(crate) fn new(lifecycle: ProcessLifecycle) -> Self {
        Self { lifecycle }
    }
}

impl UpdateTaskExecutor for LifecycleUpdateTaskExecutor {
    fn accepts_new_tasks(&self) -> bool {
        self.lifecycle.phase() == ShutdownPhase::Running
    }

    fn try_spawn(&self, task: UpdateTask) -> bool {
        if !self.accepts_new_tasks() {
            return false;
        }
        let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
        let handle = self.lifecycle.spawn_critical(async move {
            if start_receiver.await.is_ok() {
                task.await;
            }
        });
        if !self.accepts_new_tasks() {
            drop(start_sender);
            drop(handle);
            return false;
        }
        start_sender.send(()).is_ok()
    }

    fn run_blocking(&self, task: UpdateBlockingTask) -> UpdateBlockingFuture {
        let lifecycle = self.lifecycle.clone();
        Box::pin(async move {
            lifecycle.spawn_blocking(task).await.map_err(|error| {
                UpdateError::new(
                    UpdateErrorKind::InstallFailed,
                    format!("update blocking task failed: {error}"),
                )
            })?
        })
    }

    fn spawn_blocking_commit(&self, task: UpdateCommitTask) {
        drop(self.lifecycle.spawn_blocking(task));
    }
}

#[derive(Clone)]
pub(crate) struct RestartSignal {
    inner: Arc<RestartSignalInner>,
}

struct RestartSignalInner {
    token: CancellationToken,
    kind: AtomicU8,
    manual_supported: bool,
}

impl RestartSignal {
    pub(crate) fn new(manual_supported: bool) -> Self {
        Self {
            inner: Arc::new(RestartSignalInner {
                token: CancellationToken::new(),
                kind: AtomicU8::new(NO_RESTART),
                manual_supported,
            }),
        }
    }

    pub(crate) async fn wait(&self) {
        self.inner.token.cancelled().await;
    }

    pub(crate) fn kind(&self) -> Option<RestartKind> {
        match self.inner.kind.load(Ordering::Acquire) {
            NO_RESTART => None,
            value if value == RestartKind::Manual as u8 => Some(RestartKind::Manual),
            value if value == RestartKind::Update as u8 => Some(RestartKind::Update),
            _ => unreachable!("restart kind is internally bounded"),
        }
    }
}

impl RestartRequester for RestartSignal {
    fn request_restart(&self, kind: RestartKind) -> RestartRequestStatus {
        if kind == RestartKind::Manual && !self.inner.manual_supported {
            return RestartRequestStatus::Unsupported;
        }

        let requested = kind as u8;
        let previous = self.inner.kind.fetch_max(requested, Ordering::AcqRel);
        self.inner.token.cancel();
        if previous < requested {
            RestartRequestStatus::Accepted
        } else {
            RestartRequestStatus::AlreadyRequested
        }
    }
}

#[cfg(test)]
mod tests;
