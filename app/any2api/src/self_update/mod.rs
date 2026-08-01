use any2api_runtime::api::{ProcessLifecycle, ShutdownPhase};
use any2api_updater::api::{RestartRequester, UpdateTask, UpdateTaskExecutor};
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
mod tests {
    use std::{
        future,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use any2api_runtime::api::ProcessLifecycle;
    use any2api_updater::api::{RestartRequester, UpdateTaskExecutor};
    use tokio::sync::oneshot;

    use super::{LifecycleUpdateTaskExecutor, RestartSignal};

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    #[tokio::test]
    async fn restart_request_is_sticky_for_the_shutdown_waiter() {
        let signal = RestartSignal::new();
        signal.request_restart();

        tokio::time::timeout(Duration::from_millis(20), signal.wait())
            .await
            .expect("restart signal");
        assert!(signal.requested());
    }

    #[tokio::test]
    async fn draining_rejects_an_update_task_without_polling_it() {
        let lifecycle = ProcessLifecycle::new();
        let executor = LifecycleUpdateTaskExecutor::new(lifecycle.clone());
        let polled = Arc::new(AtomicBool::new(false));
        let task_polled = Arc::clone(&polled);
        lifecycle.begin_draining();

        assert!(!executor.try_spawn(Box::pin(async move {
            task_polled.store(true, Ordering::Release);
        })));
        tokio::task::yield_now().await;

        assert!(!polled.load(Ordering::Acquire));
        assert_eq!(lifecycle.background_task_count(), 0);
    }

    #[tokio::test]
    async fn forced_shutdown_cancels_and_converges_an_accepted_update_task() {
        let lifecycle = ProcessLifecycle::new();
        let executor = LifecycleUpdateTaskExecutor::new(lifecycle.clone());
        let dropped = Arc::new(AtomicBool::new(false));
        let task_dropped = Arc::clone(&dropped);
        let (started_sender, started_receiver) = oneshot::channel();

        assert!(executor.try_spawn(Box::pin(async move {
            let _drop_flag = DropFlag(task_dropped);
            started_sender.send(()).ok();
            future::pending::<()>().await;
        })));
        started_receiver.await.expect("update task started");
        assert_eq!(lifecycle.background_task_count(), 1);

        lifecycle.close_background_tasks();
        lifecycle.force();
        tokio::time::timeout(
            Duration::from_millis(100),
            lifecycle.wait_for_background_tasks(),
        )
        .await
        .expect("tracked update task converged");

        assert!(dropped.load(Ordering::Acquire));
        assert_eq!(lifecycle.background_task_count(), 0);
    }
}
