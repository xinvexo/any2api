use std::{
    future::Future,
    io,
    net::SocketAddr,
    pin::pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use any2api_runtime::api::{ProcessLifecycle, SnapshotStore};
use axum::{Router, body::Body, http::Request, response::Response, serve::Listener};
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use tokio::sync::{Notify, oneshot, watch};
use tower::Service;

use super::ShutdownTimeouts;

pub(crate) struct ServeOutcome {
    pub(crate) result: io::Result<()>,
    pub(crate) timeouts: ShutdownTimeouts,
}

pub(crate) async fn serve<L>(
    listener: L,
    app: Router,
    lifecycle: ProcessLifecycle,
    snapshots: &SnapshotStore,
    header_timeout: Duration,
    signal: impl Future<Output = ()> + Send,
) -> ServeOutcome
where
    L: Listener<Addr = SocketAddr>,
{
    serve_with_timeout_source(
        listener,
        app,
        lifecycle,
        || ShutdownTimeouts::capture(snapshots),
        header_timeout,
        signal,
    )
    .await
}

pub(super) async fn serve_with_timeout_source<L>(
    listener: L,
    app: Router,
    lifecycle: ProcessLifecycle,
    timeout_source: impl FnOnce() -> ShutdownTimeouts,
    header_timeout: Duration,
    signal: impl Future<Output = ()> + Send,
) -> ServeOutcome
where
    L: Listener<Addr = SocketAddr>,
{
    let (drain_sender, drain_receiver) = oneshot::channel();
    let mut server = Box::pin(run_server(listener, app, header_timeout, async move {
        let _ = drain_receiver.await;
    }));

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

async fn run_server<L, F>(
    listener: L,
    app: Router,
    header_timeout: Duration,
    drain: F,
) -> io::Result<()>
where
    L: Listener<Addr = SocketAddr>,
    F: Future<Output = ()> + Send,
{
    let (signal_tx, signal_rx) = watch::channel(());
    let (close_tx, close_rx) = watch::channel(());
    let mut make_service = app.into_make_service_with_connect_info::<SocketAddr>();
    let mut drain = pin!(drain);
    let mut listener = listener;

    loop {
        let (io, remote_addr) = tokio::select! {
            accepted = listener.accept() => accepted,
            () = &mut drain => {
                drop(signal_rx);
                break;
            }
        };
        let service = make_service
            .call(remote_addr)
            .await
            .unwrap_or_else(|error| match error {});
        let signal_tx = signal_tx.clone();
        let close_rx = close_rx.clone();
        tokio::spawn(handle_connection(
            io,
            service,
            header_timeout,
            signal_tx,
            close_rx,
        ));
    }

    drop(signal_tx);
    drop(close_rx);
    close_tx.closed().await;
    Ok(())
}

async fn handle_connection<I, S>(
    io: I,
    service: S,
    header_timeout: Duration,
    signal_tx: watch::Sender<()>,
    close_rx: watch::Receiver<()>,
) where
    I: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    S: Service<Request<Incoming>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let notify = Arc::new(Notify::new());
    let service = FirstRequestService::new(service, Arc::clone(&notify));
    let hyper_service = TowerToHyperService::new(service);
    let io = TokioIo::new(io);
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(header_timeout);
    builder
        .http2()
        .timer(TokioTimer::new())
        .enable_connect_protocol();
    let mut connection = pin!(builder.serve_connection_with_upgrades(io, hyper_service));
    let first_request = tokio::time::timeout(header_timeout, notify.notified());
    tokio::pin!(first_request);

    let first = tokio::select! {
        result = connection.as_mut() => {
            log_connection_result(result);
            drop(close_rx);
            return;
        }
        () = signal_tx.closed() => {
            connection.as_mut().graceful_shutdown();
            let result = connection.await;
            log_connection_result(result);
            drop(close_rx);
            return;
        }
        result = &mut first_request => result,
    };
    if first.is_err() {
        connection.as_mut().graceful_shutdown();
        let result = connection.await;
        log_connection_result(result);
        drop(close_rx);
        return;
    }

    tokio::select! {
        result = connection.as_mut() => {
            log_connection_result(result);
        }
        () = signal_tx.closed() => {
            connection.as_mut().graceful_shutdown();
            let result = connection.await;
            log_connection_result(result);
        }
    }
    drop(close_rx);
}

fn log_connection_result<E: std::fmt::Debug>(result: Result<(), E>) {
    if let Err(error) = result {
        tracing::trace!(?error, "failed to serve HTTP connection");
    }
}

#[derive(Clone)]
struct FirstRequestService<S> {
    inner: S,
    notify: Arc<Notify>,
}

impl<S> FirstRequestService<S> {
    fn new(inner: S, notify: Arc<Notify>) -> Self {
        Self { inner, notify }
    }
}

impl<S, B> Service<Request<B>> for FirstRequestService<S>
where
    S: Service<Request<B>> + Clone,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        self.notify.notify_one();
        self.inner.call(request)
    }
}
