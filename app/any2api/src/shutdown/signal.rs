#[cfg(unix)]
pub(crate) struct ShutdownSignal {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

#[cfg(unix)]
impl ShutdownSignal {
    pub(crate) fn install() -> std::io::Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};

        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
        })
    }

    pub(crate) async fn wait(mut self) {
        tokio::select! {
            signal = self.interrupt.recv() => wait_if_closed(signal, "SIGINT").await,
            signal = self.terminate.recv() => wait_if_closed(signal, "SIGTERM").await,
        }
    }
}

#[cfg(unix)]
async fn wait_if_closed(signal: Option<()>, name: &str) {
    if signal.is_none() {
        tracing::error!(%name, "shutdown signal stream closed; signal shutdown is unavailable");
        std::future::pending::<()>().await;
    }
}

#[cfg(not(unix))]
pub(crate) struct ShutdownSignal;

#[cfg(not(unix))]
impl ShutdownSignal {
    pub(crate) fn install() -> std::io::Result<Self> {
        Ok(Self)
    }

    pub(crate) async fn wait(self) {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to listen for Ctrl-C; signal shutdown is unavailable");
            std::future::pending::<()>().await;
        }
    }
}
