use std::{
    future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use any2api_runtime::api::ProcessLifecycle;
use any2api_updater::api::{
    RestartKind, RestartRequestStatus, RestartRequester, UpdateTaskExecutor,
};
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
    let signal = RestartSignal::new(true);
    assert_eq!(
        signal.request_restart(RestartKind::Manual),
        RestartRequestStatus::Accepted
    );

    tokio::time::timeout(Duration::from_millis(20), signal.wait())
        .await
        .expect("restart signal");
    assert_eq!(signal.kind(), Some(RestartKind::Manual));
}

#[test]
fn duplicate_manual_restart_is_idempotent() {
    let signal = RestartSignal::new(true);

    assert_eq!(
        signal.request_restart(RestartKind::Manual),
        RestartRequestStatus::Accepted
    );
    assert_eq!(
        signal.request_restart(RestartKind::Manual),
        RestartRequestStatus::AlreadyRequested
    );
    assert_eq!(signal.kind(), Some(RestartKind::Manual));
}

#[test]
fn update_restart_promotes_a_manual_restart() {
    let signal = RestartSignal::new(true);
    signal.request_restart(RestartKind::Manual);

    assert_eq!(
        signal.request_restart(RestartKind::Update),
        RestartRequestStatus::Accepted
    );
    assert_eq!(
        signal.request_restart(RestartKind::Manual),
        RestartRequestStatus::AlreadyRequested
    );
    assert_eq!(signal.kind(), Some(RestartKind::Update));
}

#[test]
fn unsupported_manual_restart_does_not_signal_shutdown() {
    let signal = RestartSignal::new(false);

    assert_eq!(
        signal.request_restart(RestartKind::Manual),
        RestartRequestStatus::Unsupported
    );
    assert_eq!(signal.kind(), None);
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

#[tokio::test]
async fn forced_shutdown_keeps_a_started_update_commit_tracked() {
    let lifecycle = ProcessLifecycle::new();
    let executor = LifecycleUpdateTaskExecutor::new(lifecycle.clone());
    let commit_executor = executor.clone();
    let finished = Arc::new(AtomicBool::new(false));
    let commit_finished = Arc::clone(&finished);
    let (started_sender, started_receiver) = oneshot::channel();
    let (release_sender, release_receiver) = std::sync::mpsc::channel();

    assert!(executor.try_spawn(Box::pin(async move {
        commit_executor.spawn_blocking_commit(Box::new(move || {
            started_sender.send(()).ok();
            release_receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("release update commit");
            commit_finished.store(true, Ordering::Release);
        }));
        future::pending::<()>().await;
    })));
    started_receiver.await.expect("update commit started");

    assert!(!finished.load(Ordering::Acquire));
    assert_eq!(lifecycle.background_task_count(), 2);
    lifecycle.close_background_tasks();
    lifecycle.force();
    tokio::time::timeout(Duration::from_millis(100), async {
        while lifecycle.background_task_count() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("outer update future should converge");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            lifecycle.wait_for_background_tasks(),
        )
        .await
        .is_err()
    );

    release_sender.send(()).expect("release update commit");
    tokio::time::timeout(
        Duration::from_millis(100),
        lifecycle.wait_for_background_tasks(),
    )
    .await
    .expect("blocking update commit should finish");
    assert!(finished.load(Ordering::Acquire));
    assert_eq!(lifecycle.background_task_count(), 0);
}

#[tokio::test]
async fn forced_shutdown_keeps_started_update_preparation_tracked() {
    let lifecycle = ProcessLifecycle::new();
    let executor = LifecycleUpdateTaskExecutor::new(lifecycle.clone());
    let preparation_executor = executor.clone();
    let finished = Arc::new(AtomicBool::new(false));
    let preparation_finished = Arc::clone(&finished);
    let (started_sender, started_receiver) = oneshot::channel();
    let (release_sender, release_receiver) = std::sync::mpsc::channel();

    assert!(executor.try_spawn(Box::pin(async move {
        preparation_executor
            .run_blocking(Box::new(move || {
                started_sender.send(()).ok();
                release_receiver
                    .recv_timeout(Duration::from_secs(1))
                    .expect("release update preparation");
                preparation_finished.store(true, Ordering::Release);
                Ok(())
            }))
            .await
            .expect("update preparation");
    })));
    started_receiver.await.expect("update preparation started");

    assert!(!finished.load(Ordering::Acquire));
    assert_eq!(lifecycle.background_task_count(), 2);
    lifecycle.close_background_tasks();
    lifecycle.force();
    tokio::time::timeout(Duration::from_millis(100), async {
        while lifecycle.background_task_count() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("outer update future should converge");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            lifecycle.wait_for_background_tasks(),
        )
        .await
        .is_err()
    );

    release_sender.send(()).expect("release update preparation");
    tokio::time::timeout(
        Duration::from_millis(100),
        lifecycle.wait_for_background_tasks(),
    )
    .await
    .expect("blocking update preparation should finish");
    assert!(finished.load(Ordering::Acquire));
    assert_eq!(lifecycle.background_task_count(), 0);
}
