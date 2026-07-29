use any2api_updater::api::RestartRequester;
use tokio_util::sync::CancellationToken;

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
    use std::time::Duration;

    use any2api_updater::api::RestartRequester;

    use super::RestartSignal;

    #[tokio::test]
    async fn restart_request_is_sticky_for_the_shutdown_waiter() {
        let signal = RestartSignal::new();
        signal.request_restart();

        tokio::time::timeout(Duration::from_millis(20), signal.wait())
            .await
            .expect("restart signal");
        assert!(signal.requested());
    }
}
