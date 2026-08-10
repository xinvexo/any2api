use std::{
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use http::Uri;
use hyper::rt::{Read, ReadBufCursor, Write};
use hyper_util::{
    client::legacy::connect::{Connected, Connection},
    rt::TokioIo,
};
use tokio::{net::TcpStream, time::timeout};
use tower_service::Service;

use crate::{
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    profile::GENERIC_GATEWAY_TRANSPORT_PROFILE as WIRE_PROFILE,
};

#[derive(Clone)]
pub(crate) struct ProxyTcpConnector {
    host: Arc<str>,
    port: u16,
    timeout: Duration,
    proxied: bool,
}

impl ProxyTcpConnector {
    pub(crate) fn new(host: &str, port: u16, timeout: Duration, proxied: bool) -> Self {
        Self {
            host: Arc::from(host.to_owned()),
            port,
            timeout,
            proxied,
        }
    }
}

impl Service<Uri> for ProxyTcpConnector {
    type Response = ProxyTcpStream;
    type Error = ProxyConnectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _destination: Uri) -> Self::Future {
        let host = Arc::clone(&self.host);
        let port = self.port;
        let connect_timeout = self.timeout;
        let proxied = self.proxied;
        Box::pin(async move {
            let stream = timeout(connect_timeout, TcpStream::connect((host.as_ref(), port)))
                .await
                .map_err(|_| ProxyConnectError)?
                .map_err(|_| ProxyConnectError)?;
            if WIRE_PROFILE.pinned_tcp_nodelay() {
                let _ = stream.set_nodelay(true);
            }
            // Same probing cadence as the reqwest path's tcp_keepalive so h1
            // connections through proxies also detect NAT silent drops.
            let keepalive = socket2::TcpKeepalive::new()
                .with_time(WIRE_PROFILE.tcp_keep_alive_interval())
                .with_interval(WIRE_PROFILE.tcp_keep_alive_interval());
            let _ = socket2::SockRef::from(&stream).set_tcp_keepalive(&keepalive);
            Ok(ProxyTcpStream {
                inner: TokioIo::new(stream),
                proxied,
            })
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error("configured proxy connection failed")]
pub(crate) struct ProxyConnectError;

pub(crate) struct ProxyTcpStream {
    inner: TokioIo<TcpStream>,
    proxied: bool,
}

impl Connection for ProxyTcpStream {
    fn connected(&self) -> Connected {
        Connected::new().proxy(self.proxied)
    }
}

impl Read for ProxyTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: ReadBufCursor<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_read(context, buffer)
    }
}

impl Write for ProxyTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_write(context, buffer)
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(context)
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(context)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_write_vectored(context, buffers)
    }
}

pub(crate) fn proxy_uri(host: &str, port: u16) -> Result<Uri, TransportError> {
    let authority = if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    Uri::builder()
        .scheme("http")
        .authority(authority)
        .path_and_query("/")
        .build()
        .map_err(|_| {
            TransportError::configuration(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                "configured proxy address is invalid",
            )
        })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use http::Uri;
    use tokio::net::TcpListener;
    use tower_service::Service;

    use super::ProxyTcpConnector;

    #[tokio::test]
    async fn connected_streams_enable_tcp_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
        let address = listener.local_addr().expect("listener address");
        let mut connector =
            ProxyTcpConnector::new("127.0.0.1", address.port(), Duration::from_secs(1), false);

        let stream = connector
            .call(Uri::from_static("http://ignored.invalid/"))
            .await
            .expect("TCP connection");

        let keepalive = socket2::SockRef::from(stream.inner.inner())
            .keepalive()
            .expect("keepalive query");
        assert!(keepalive);
    }
}
