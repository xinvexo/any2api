use std::future::Future;

pub(crate) async fn run<T>(future: impl Future<Output = T> + Send + 'static) -> T
where
    T: Send + 'static,
{
    tokio::spawn(future)
        .await
        .expect("configuration publish task must run to completion")
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::oneshot;

    use super::run;

    #[tokio::test]
    async fn cancelled_waiter_does_not_cancel_the_publish_task() {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let (completed_tx, completed_rx) = oneshot::channel();
        let waiter = tokio::spawn(async move {
            run(async move {
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
    }
}
