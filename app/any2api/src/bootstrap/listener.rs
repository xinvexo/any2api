use std::{
    io,
    num::NonZeroUsize,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::serve::{Listener, ListenerExt, TapIo};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{OwnedSemaphorePermit, Semaphore},
};

pub(super) const DEFAULT_MAX_CONNECTIONS: NonZeroUsize = NonZeroUsize::new(4_096).unwrap();

/// Builds the inbound listener: a concurrent-connection cap plus TCP_NODELAY
/// on every accepted stream.
///
/// The `tap_io` wrapper must stay outermost: axum only implements
/// `Connected<IncomingStream<...>>` for `SocketAddr` on `TcpListener` and
/// `TapIo`, and the access log middleware relies on that extraction.
pub(super) fn inbound_listener(
    listener: TcpListener,
    max_connections: NonZeroUsize,
) -> TapIo<LimitedListener<TcpListener>, fn(&mut LimitedIo<TcpStream>)> {
    LimitedListener::new(listener, max_connections).tap_io(enable_nodelay)
}

fn enable_nodelay(io: &mut LimitedIo<TcpStream>) {
    if let Err(error) = io.get_mut().set_nodelay(true) {
        tracing::debug!(%error, "failed to enable TCP_NODELAY on an inbound connection");
    }
}

/// Caps the number of concurrently served connections.
///
/// `accept` waits for a semaphore permit before accepting, so once the limit
/// is reached new connections queue in the kernel backlog instead of
/// allocating per-connection state.
pub(super) struct LimitedListener<L> {
    inner: L,
    permits: Arc<Semaphore>,
}

impl<L> LimitedListener<L> {
    pub(super) fn new(inner: L, max_connections: NonZeroUsize) -> Self {
        Self {
            inner,
            permits: Arc::new(Semaphore::new(max_connections.get())),
        }
    }
}

impl<L> Listener for LimitedListener<L>
where
    L: Listener,
{
    type Io = LimitedIo<L::Io>;
    type Addr = L::Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .expect("connection limit semaphore is never closed");
        let (io, addr) = self.inner.accept().await;
        (
            LimitedIo {
                inner: io,
                _permit: permit,
            },
            addr,
        )
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

/// Connection IO holding its concurrency permit; dropping the connection
/// releases the permit and lets `accept` resume.
pub(super) struct LimitedIo<Io> {
    inner: Io,
    _permit: OwnedSemaphorePermit,
}

impl<Io> LimitedIo<Io> {
    pub(super) fn get_mut(&mut self) -> &mut Io {
        &mut self.inner
    }
}

impl<Io> AsyncRead for LimitedIo<Io>
where
    Io: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<Io> AsyncWrite for LimitedIo<Io>
where
    Io: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, time::Duration};

    use axum::serve::Listener;
    use tokio::net::{TcpListener, TcpStream};

    use super::{DEFAULT_MAX_CONNECTIONS, LimitedListener, inbound_listener};

    #[tokio::test]
    async fn accept_pauses_at_the_connection_limit_until_a_permit_is_released() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let mut limited = LimitedListener::new(listener, NonZeroUsize::MIN);
        let _first_client = TcpStream::connect(address).await.expect("first client");
        let (first_io, _) = limited.accept().await;

        let _second_client = TcpStream::connect(address).await.expect("second client");
        let delayed = tokio::time::timeout(Duration::from_millis(50), limited.accept()).await;
        assert!(
            delayed.is_err(),
            "accept must pause while the permit is held"
        );

        drop(first_io);
        tokio::time::timeout(Duration::from_secs(5), limited.accept())
            .await
            .expect("accept resumes after the permit is released");
    }

    #[tokio::test]
    async fn inbound_listener_enables_nodelay_on_accepted_connections() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("address");
        let mut listener = inbound_listener(listener, DEFAULT_MAX_CONNECTIONS);
        let _client = TcpStream::connect(address).await.expect("client");
        let (mut io, _) = listener.accept().await;
        assert!(io.get_mut().nodelay().expect("nodelay"));
    }
}
