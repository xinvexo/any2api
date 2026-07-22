use std::{
    future::{Future, IntoFuture},
    io,
};

use any2api_runtime::api::{ProcessLifecycle, SnapshotStore};
use axum::Router;
use tokio::{net::TcpListener, sync::oneshot};

use super::ShutdownTimeouts;

pub(crate) struct ServeOutcome {
    pub(crate) result: io::Result<()>,
    pub(crate) timeouts: ShutdownTimeouts,
}

pub(crate) async fn serve(
    listener: TcpListener,
    app: Router,
    lifecycle: ProcessLifecycle,
    snapshots: &SnapshotStore,
    signal: impl Future<Output = ()> + Send,
) -> ServeOutcome {
    serve_with_timeout_source(
        listener,
        app,
        lifecycle,
        || ShutdownTimeouts::capture(snapshots),
        signal,
    )
    .await
}

pub(super) async fn serve_with_timeout_source(
    listener: TcpListener,
    app: Router,
    lifecycle: ProcessLifecycle,
    timeout_source: impl FnOnce() -> ShutdownTimeouts,
    signal: impl Future<Output = ()> + Send,
) -> ServeOutcome {
    let (drain_sender, drain_receiver) = oneshot::channel();
    let mut server = Box::pin(
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = drain_receiver.await;
        })
        .into_future(),
    );

    let ended_before_signal = tokio::select! {
        result = server.as_mut() => Some(result),
        () = signal => None,
    };
    let timeouts = timeout_source();
    lifecycle.begin_draining();

    if let Some(result) = ended_before_signal {
        return ServeOutcome { result, timeouts };
    }

    tracing::info!(
        active_requests = lifecycle.active_requests(),
        "shutdown signal received; draining HTTP requests"
    );
    drain_sender.send(()).ok();

    let result = match tokio::time::timeout(timeouts.request_grace, server.as_mut()).await {
        Ok(result) => result,
        Err(_) => {
            lifecycle.force();
            tracing::warn!(
                active_requests = lifecycle.active_requests(),
                "HTTP request grace period expired; forcing cancellation"
            );
            match tokio::time::timeout(timeouts.finalize, server.as_mut()).await {
                Ok(result) => result,
                Err(_) => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "HTTP server did not stop after forced cancellation",
                )),
            }
        }
    };
    ServeOutcome { result, timeouts }
}
