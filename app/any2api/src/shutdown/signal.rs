pub(crate) async fn signal() {
    #[cfg(unix)]
    {
        tokio::select! {
            () = ctrl_c() => {}
            () = sigterm() => {}
        }
    }
    #[cfg(not(unix))]
    ctrl_c().await;
}

async fn ctrl_c() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to listen for Ctrl-C; signal shutdown is unavailable");
        std::future::pending::<()>().await;
    }
}

#[cfg(unix)]
async fn sigterm() {
    let mut stream = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        Ok(stream) => stream,
        Err(error) => {
            tracing::error!(%error, "failed to listen for SIGTERM; signal shutdown is unavailable");
            std::future::pending::<()>().await;
            unreachable!("pending signal fallback does not complete")
        }
    };
    if stream.recv().await.is_none() {
        tracing::error!("SIGTERM signal stream closed; signal shutdown is unavailable");
        std::future::pending::<()>().await;
    }
}
