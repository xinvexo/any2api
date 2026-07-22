use std::future::Future;

use crate::process_lifecycle::ProcessLifecycle;

pub(crate) async fn run<T>(
    lifecycle: ProcessLifecycle,
    future: impl Future<Output = T> + Send + 'static,
) -> Option<T>
where
    T: Send + 'static,
{
    lifecycle
        .spawn_critical(future)
        .await
        .expect("configuration publish task must run to completion")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::{ProcessLifecycle, run};

    #[tokio::test]
    async fn cancelled_waiter_does_not_cancel_the_publish_task() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let (completed_tx, completed_rx) = oneshot::channel();
        let lifecycle = ProcessLifecycle::new();
        let task_lifecycle = lifecycle.clone();
        let waiter = tokio::spawn(async move {
            let _ = run(task_lifecycle, async move {
                started_tx.send(()).ok();
                release_rx.await.expect("release publish task");
                completed_tx.send(()).ok();
            })
            .await;
        });

        started_rx.await.expect("publish task started");
        waiter.abort();
        release_tx.send(()).expect("release detached publish task");

        tokio::time::timeout(Duration::from_secs(1), completed_rx)
            .await
            .expect("publish task must outlive its waiter")
            .expect("publish task completed");
        lifecycle.close_background_tasks();
        lifecycle.wait_for_background_tasks().await;
    }

    #[tokio::test]
    async fn forced_shutdown_cancels_the_detached_publish_task() {
        let lifecycle = ProcessLifecycle::new();
        let task_lifecycle = lifecycle.clone();
        let task =
            tokio::spawn(async move { run(task_lifecycle, std::future::pending::<()>()).await });
        tokio::task::yield_now().await;

        lifecycle.force();
        lifecycle.close_background_tasks();

        assert_eq!(task.await.expect("publish waiter"), None);
        lifecycle.wait_for_background_tasks().await;
    }
}
