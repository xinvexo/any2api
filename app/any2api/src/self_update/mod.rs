use any2api_runtime::api::{ProcessLifecycle, ShutdownPhase};
use any2api_updater::api::{
    RestartRequester, UpdateBlockingFuture, UpdateBlockingTask, UpdateCommitTask, UpdateError,
    UpdateErrorKind, UpdateTask, UpdateTaskExecutor,
};
use tokio_util::sync::CancellationToken;

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

#[derive(Clone, Default)]
pub(crate) struct RestartSignal {
    token: CancellationToken,
}

impl RestartSignal {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) async fn wait(&self) {
        self.token.cancelled().await;
    }

    pub(crate) fn requested(&self) -> bool {
        self.token.is_cancelled()
    }
}

impl RestartRequester for RestartSignal {
    fn request_restart(&self) {
        self.token.cancel();
    }
}

#[cfg(test)]
mod tests;
