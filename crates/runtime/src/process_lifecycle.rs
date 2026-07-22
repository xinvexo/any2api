use std::{
    fmt,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

use tokio::task::JoinHandle;
use tokio_util::{
    sync::CancellationToken,
    task::{TaskTracker, task_tracker::TaskTrackerToken},
};

const RUNNING: u8 = 0;
const DRAINING: u8 = 1;
const FORCED: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShutdownPhase {
    Running,
    Draining,
    Forced,
}

impl ShutdownPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Draining => "draining",
            Self::Forced => "forced",
        }
    }
}

#[derive(Clone)]
pub struct ProcessLifecycle {
    inner: Arc<LifecycleInner>,
}

struct LifecycleInner {
    phase: AtomicU8,
    requests: TaskTracker,
    background: TaskTracker,
    draining: CancellationToken,
    forced: CancellationToken,
}

impl ProcessLifecycle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(LifecycleInner {
                phase: AtomicU8::new(RUNNING),
                requests: TaskTracker::new(),
                background: TaskTracker::new(),
                draining: CancellationToken::new(),
                forced: CancellationToken::new(),
            }),
        }
    }

    #[must_use]
    pub fn phase(&self) -> ShutdownPhase {
        match self.inner.phase.load(Ordering::Acquire) {
            RUNNING => ShutdownPhase::Running,
            DRAINING => ShutdownPhase::Draining,
            FORCED => ShutdownPhase::Forced,
            _ => unreachable!("shutdown phase is internally bounded"),
        }
    }

    pub fn begin_draining(&self) -> bool {
        if self
            .inner
            .phase
            .compare_exchange(RUNNING, DRAINING, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        self.inner.requests.close();
        self.inner.draining.cancel();
        true
    }

    pub fn force(&self) -> bool {
        let previous = self.inner.phase.swap(FORCED, Ordering::AcqRel);
        if previous == FORCED {
            return false;
        }
        self.inner.requests.close();
        self.inner.draining.cancel();
        self.inner.forced.cancel();
        true
    }

    #[must_use]
    pub fn track_request(&self) -> Option<ActiveRequestGuard> {
        if self.phase() != ShutdownPhase::Running {
            return None;
        }
        let token = self.inner.requests.token();
        if self.phase() != ShutdownPhase::Running {
            return None;
        }
        Some(ActiveRequestGuard { _token: token })
    }

    #[must_use]
    pub fn active_requests(&self) -> usize {
        self.inner.requests.len()
    }

    pub async fn wait_for_requests(&self) {
        self.inner.requests.wait().await;
    }

    pub async fn forced(&self) {
        self.inner.forced.cancelled().await;
    }

    pub fn spawn_critical<F>(&self, future: F) -> JoinHandle<Option<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let forced = self.inner.forced.clone();
        self.inner.background.spawn(async move {
            tokio::select! {
                output = future => Some(output),
                () = forced.cancelled() => None,
            }
        })
    }

    pub(crate) fn spawn_until_draining<F>(&self, future: F) -> JoinHandle<Option<F::Output>>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let draining = self.inner.draining.clone();
        self.inner.background.spawn(async move {
            tokio::select! {
                output = future => Some(output),
                () = draining.cancelled() => None,
            }
        })
    }

    pub(crate) fn spawn_tracked<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.inner.background.spawn(future)
    }

    pub fn spawn_blocking<F, T>(&self, task: F) -> JoinHandle<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.inner.background.spawn_blocking(task)
    }

    pub fn close_background_tasks(&self) {
        self.inner.background.close();
    }

    #[must_use]
    pub fn background_task_count(&self) -> usize {
        self.inner.background.len()
    }

    pub async fn wait_for_background_tasks(&self) {
        self.inner.background.wait().await;
    }
}

impl Default for ProcessLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ProcessLifecycle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProcessLifecycle")
            .field("phase", &self.phase())
            .field("active_requests", &self.active_requests())
            .field("background_tasks", &self.background_task_count())
            .finish()
    }
}

pub struct ActiveRequestGuard {
    _token: TaskTrackerToken,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::{ProcessLifecycle, ShutdownPhase};

    #[tokio::test]
    async fn request_guard_keeps_draining_open_until_drop() {
        let lifecycle = ProcessLifecycle::new();
        let guard = lifecycle.track_request().expect("running request");
        assert!(lifecycle.begin_draining());
        assert_eq!(lifecycle.phase(), ShutdownPhase::Draining);

        assert!(
            tokio::time::timeout(Duration::from_millis(10), lifecycle.wait_for_requests())
                .await
                .is_err()
        );
        drop(guard);
        lifecycle.wait_for_requests().await;
    }

    #[test]
    fn draining_rejects_new_request_guards() {
        let lifecycle = ProcessLifecycle::new();
        lifecycle.begin_draining();

        assert!(lifecycle.track_request().is_none());
    }

    #[tokio::test]
    async fn blocking_task_remains_tracked_after_join_handle_drop() {
        let lifecycle = ProcessLifecycle::new();
        let (started_sender, started_receiver) = std::sync::mpsc::channel();
        let (release_sender, release_receiver) = std::sync::mpsc::channel();
        let task = lifecycle.spawn_blocking(move || {
            started_sender.send(()).expect("started");
            release_receiver.recv().expect("release");
        });
        started_receiver.recv().expect("blocking task started");
        drop(task);
        lifecycle.close_background_tasks();

        assert!(
            tokio::time::timeout(
                Duration::from_millis(10),
                lifecycle.wait_for_background_tasks()
            )
            .await
            .is_err()
        );
        release_sender.send(()).expect("release blocking task");
        lifecycle.wait_for_background_tasks().await;
    }

    #[tokio::test]
    async fn forced_shutdown_cancels_tracked_critical_tasks() {
        let lifecycle = ProcessLifecycle::new();
        let (_sender, receiver) = oneshot::channel::<()>();
        let task = lifecycle.spawn_critical(async move {
            receiver.await.ok();
        });
        lifecycle.close_background_tasks();
        assert!(lifecycle.force());

        assert_eq!(task.await.expect("tracked task"), None);
        lifecycle.wait_for_background_tasks().await;
        assert_eq!(lifecycle.phase(), ShutdownPhase::Forced);
    }

    #[tokio::test]
    async fn draining_stops_health_style_background_tasks() {
        let lifecycle = ProcessLifecycle::new();
        let task = lifecycle.spawn_until_draining(std::future::pending::<()>());
        lifecycle.close_background_tasks();
        lifecycle.begin_draining();

        assert_eq!(task.await.expect("tracked task"), None);
        lifecycle.wait_for_background_tasks().await;
    }
}
