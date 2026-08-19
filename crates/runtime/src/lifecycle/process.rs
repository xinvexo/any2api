use std::{
    fmt,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use tokio::{sync::Notify, task::JoinHandle};
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MemoryReclamationMetrics {
    blockers: usize,
    completed_runs: u64,
    last_duration_micros: u64,
}

impl MemoryReclamationMetrics {
    #[must_use]
    pub const fn blockers(self) -> usize {
        self.blockers
    }

    #[must_use]
    pub const fn completed_runs(self) -> u64 {
        self.completed_runs
    }

    #[must_use]
    pub const fn last_duration_micros(self) -> u64 {
        self.last_duration_micros
    }
}

#[derive(Clone)]
pub struct ProcessLifecycle {
    inner: Arc<LifecycleInner>,
}

struct LifecycleInner {
    phase: AtomicU8,
    activity_epoch: AtomicU64,
    memory_reclamation: Arc<MemoryReclamationState>,
    requests: TaskTracker,
    background: TaskTracker,
    draining: CancellationToken,
    forced: CancellationToken,
}

#[derive(Default)]
struct MemoryReclamationState {
    blockers: AtomicUsize,
    requested: Notify,
    completed_runs: AtomicU64,
    last_duration_micros: AtomicU64,
}

impl ProcessLifecycle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(LifecycleInner {
                phase: AtomicU8::new(RUNNING),
                activity_epoch: AtomicU64::new(0),
                memory_reclamation: Arc::new(MemoryReclamationState::default()),
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
        let memory_reclamation =
            MemoryReclamationBlocker::new(Arc::clone(&self.inner.memory_reclamation));
        let token = self.inner.requests.token();
        if self.phase() != ShutdownPhase::Running {
            return None;
        }
        self.record_activity();
        Some(ActiveRequestGuard {
            _token: token,
            memory_reclamation,
        })
    }

    #[must_use]
    pub fn active_requests(&self) -> usize {
        self.inner.requests.len()
    }

    #[must_use]
    pub fn activity_epoch(&self) -> u64 {
        self.inner.activity_epoch.load(Ordering::Acquire)
    }

    pub fn record_activity(&self) {
        self.inner.activity_epoch.fetch_add(1, Ordering::AcqRel);
        if self.memory_reclamation_blockers() == 0 {
            self.inner.memory_reclamation.requested.notify_one();
        }
    }

    #[must_use]
    pub fn memory_reclamation_blockers(&self) -> usize {
        self.inner
            .memory_reclamation
            .blockers
            .load(Ordering::Acquire)
    }

    pub async fn memory_reclamation_requested(&self) {
        self.inner.memory_reclamation.requested.notified().await;
    }

    pub fn record_memory_reclamation(&self, duration: Duration) {
        self.inner.memory_reclamation.last_duration_micros.store(
            duration.as_micros().try_into().unwrap_or(u64::MAX),
            Ordering::Release,
        );
        self.inner
            .memory_reclamation
            .completed_runs
            .fetch_add(1, Ordering::Release);
    }

    #[must_use]
    pub fn memory_reclamation_metrics(&self) -> MemoryReclamationMetrics {
        MemoryReclamationMetrics {
            blockers: self.memory_reclamation_blockers(),
            completed_runs: self
                .inner
                .memory_reclamation
                .completed_runs
                .load(Ordering::Acquire),
            last_duration_micros: self
                .inner
                .memory_reclamation
                .last_duration_micros
                .load(Ordering::Acquire),
        }
    }

    pub async fn wait_for_requests(&self) {
        self.inner.requests.wait().await;
    }

    pub async fn draining(&self) {
        self.inner.draining.cancelled().await;
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

    /// Spawn a background task that is cancelled when process draining begins.
    ///
    /// Server-side infrastructure uses this boundary for non-critical shared
    /// samplers. The task remains in the lifecycle tracker for orderly shutdown.
    pub fn spawn_until_draining<F>(&self, future: F) -> JoinHandle<Option<F::Output>>
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
            .field(
                "memory_reclamation_blockers",
                &self.memory_reclamation_blockers(),
            )
            .field("background_tasks", &self.background_task_count())
            .finish()
    }
}

pub struct ActiveRequestGuard {
    _token: TaskTrackerToken,
    memory_reclamation: MemoryReclamationBlocker,
}

impl ActiveRequestGuard {
    pub fn release_memory_reclamation_blocker(&mut self) {
        self.memory_reclamation.release();
    }
}

struct MemoryReclamationBlocker {
    state: Option<Arc<MemoryReclamationState>>,
}

impl MemoryReclamationBlocker {
    fn new(state: Arc<MemoryReclamationState>) -> Self {
        state.blockers.fetch_add(1, Ordering::AcqRel);
        Self { state: Some(state) }
    }

    fn release(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        let previous = state.blockers.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "memory reclamation blocker underflow");
        if previous == 1 {
            state.requested.notify_one();
        }
    }
}

impl Drop for MemoryReclamationBlocker {
    fn drop(&mut self) {
        self.release();
    }
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
        assert_eq!(lifecycle.memory_reclamation_blockers(), 1);
        assert!(lifecycle.begin_draining());
        assert_eq!(lifecycle.phase(), ShutdownPhase::Draining);

        assert!(
            tokio::time::timeout(Duration::from_millis(10), lifecycle.wait_for_requests())
                .await
                .is_err()
        );
        drop(guard);
        assert_eq!(lifecycle.memory_reclamation_blockers(), 0);
        lifecycle.wait_for_requests().await;
    }

    #[tokio::test]
    async fn draining_signal_is_visible_to_noncritical_streams() {
        let lifecycle = ProcessLifecycle::new();
        let waiting = lifecycle.clone();
        let task = tokio::spawn(async move { waiting.draining().await });

        assert!(lifecycle.begin_draining());
        task.await.expect("draining observer");
    }

    #[test]
    fn draining_rejects_new_request_guards() {
        let lifecycle = ProcessLifecycle::new();
        lifecycle.begin_draining();

        assert!(lifecycle.track_request().is_none());
        assert_eq!(lifecycle.activity_epoch(), 0);
        assert_eq!(lifecycle.memory_reclamation_blockers(), 0);
    }

    #[test]
    fn accepted_requests_advance_the_shared_activity_epoch() {
        let lifecycle = ProcessLifecycle::new();
        let observer = lifecycle.clone();
        assert_eq!(observer.activity_epoch(), 0);

        let first = lifecycle.track_request().expect("first request");
        let mut second = lifecycle.track_request().expect("second request");
        assert_eq!(observer.activity_epoch(), 2);
        assert_eq!(observer.memory_reclamation_blockers(), 2);

        second.release_memory_reclamation_blocker();
        assert_eq!(observer.active_requests(), 2);
        assert_eq!(observer.memory_reclamation_blockers(), 1);
        drop((first, second));
        assert_eq!(observer.activity_epoch(), 2);
        assert_eq!(observer.active_requests(), 0);
        assert_eq!(observer.memory_reclamation_blockers(), 0);
    }

    #[test]
    fn background_activity_advances_the_shared_activity_epoch() {
        let lifecycle = ProcessLifecycle::new();

        lifecycle.record_activity();

        assert_eq!(lifecycle.activity_epoch(), 1);
    }

    #[test]
    fn memory_reclamation_metrics_report_blockers_runs_and_duration() {
        let lifecycle = ProcessLifecycle::new();
        let guard = lifecycle.track_request().expect("request");
        lifecycle.record_memory_reclamation(Duration::from_micros(725));

        assert_eq!(
            lifecycle.memory_reclamation_metrics(),
            super::MemoryReclamationMetrics {
                blockers: 1,
                completed_runs: 1,
                last_duration_micros: 725,
            }
        );

        drop(guard);
        assert_eq!(lifecycle.memory_reclamation_metrics().blockers(), 0);
    }

    #[tokio::test]
    async fn unblocked_background_activity_notifies_memory_reclamation() {
        let lifecycle = ProcessLifecycle::new();

        lifecycle.record_activity();

        tokio::time::timeout(
            Duration::from_millis(10),
            lifecycle.memory_reclamation_requested(),
        )
        .await
        .expect("memory reclamation notification");
    }

    #[tokio::test]
    async fn only_the_last_request_blocker_notifies_memory_reclamation() {
        let lifecycle = ProcessLifecycle::new();
        let first = lifecycle.track_request().expect("first request");
        let mut second = lifecycle.track_request().expect("second request");
        let notification = lifecycle.memory_reclamation_requested();
        tokio::pin!(notification);

        second.release_memory_reclamation_blocker();
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut notification)
                .await
                .is_err()
        );

        drop(first);
        tokio::time::timeout(Duration::from_millis(10), notification)
            .await
            .expect("last blocker notification");
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
