use std::{
    error::Error as StdError,
    fmt,
    future::{Future, poll_fn},
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::ProxyKind;
use futures_util::{StreamExt, stream::FuturesUnordered};
use http::{HeaderValue, Uri};
use hyper_rustls::{HttpsConnector, MaybeHttpsStream};
use hyper_util::client::legacy::connect::proxy::{SocksV5, Tunnel};
use rustls::{ClientConfig, pki_types::ServerName};
use tokio::time::timeout;
use tower_service::Service;

use super::tls::wrap_tls;
use crate::{
    api::TransportProxy,
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    proxy::tcp::{ProxyTcpConnector, ProxyTcpStream, proxy_uri},
    resolution::{DnsLookupError, OriginTarget, shared_dns_cache},
};

type HttpTunnelConnector = ResolvedTarget<HttpsConnector<Tunnel<ProxyTcpConnector>>>;
type SocksConnector = ResolvedTarget<HttpsConnector<SocksV5<ProxyTcpConnector>>>;

pub(crate) type PinnedIo = MaybeHttpsStream<ProxyTcpStream>;

#[derive(Clone)]
pub(crate) struct PinnedConnector {
    inner: PinnedConnectorInner,
    connect_timeout: Duration,
}

#[derive(Clone)]
enum PinnedConnectorInner {
    HttpTunnel(HttpTunnelConnector),
    Socks(SocksConnector),
}

impl PinnedConnector {
    pub(crate) fn build(
        connect_timeout: Duration,
        tls_config: ClientConfig,
        proxy: TransportProxy<'_>,
        origin: &OriginTarget,
        proxy_authorization: Option<HeaderValue>,
    ) -> Result<Self, TransportError> {
        let profile = proxy.profile();
        let address = profile.address().ok_or_else(|| {
            TransportError::configuration(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                "configured proxy has no network address",
            )
        })?;
        let server_name = ServerName::try_from(origin.host.to_string()).map_err(|_| {
            TransportError::configuration(
                TransportErrorStage::Tls,
                TransportFailureScope::EgressPath,
                "upstream TLS server name is invalid",
            )
        })?;

        match profile.kind() {
            ProxyKind::Http => {
                // Tunnel cleartext origins too: CONNECT can race every pinned
                // address before Hyper is allowed to poll the request body.
                let tcp =
                    ProxyTcpConnector::new(address.host(), address.port(), connect_timeout, false);
                let mut tunnel = Tunnel::new(proxy_uri(address.host(), address.port())?, tcp);
                if let Some(value) = proxy_authorization {
                    tunnel = tunnel.with_auth(value);
                }
                Ok(Self {
                    inner: PinnedConnectorInner::HttpTunnel(ResolvedTarget::new(
                        wrap_tls(tunnel, tls_config, server_name),
                        origin,
                    )),
                    connect_timeout,
                })
            }
            ProxyKind::Socks5 => {
                let tcp =
                    ProxyTcpConnector::new(address.host(), address.port(), connect_timeout, false);
                let mut socks = SocksV5::new(proxy_uri(address.host(), address.port())?, tcp);
                if let Some(credentials) = proxy.credentials() {
                    socks = socks.with_auth(
                        credentials.username().to_owned(),
                        credentials.password().to_owned(),
                    );
                }
                Ok(Self {
                    inner: PinnedConnectorInner::Socks(ResolvedTarget::new(
                        wrap_tls(socks, tls_config, server_name),
                        origin,
                    )),
                    connect_timeout,
                })
            }
            ProxyKind::Direct => Err(TransportError::configuration(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Unattributed,
                "pinned proxy connector cannot use DIRECT",
            )),
        }
    }
}

impl fmt::Debug for PinnedConnector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PinnedConnector")
            .field(&match self.inner {
                PinnedConnectorInner::HttpTunnel(_) => "http_tunnel",
                PinnedConnectorInner::Socks(_) => "socks5",
            })
            .finish()
    }
}

impl Service<Uri> for PinnedConnector {
    type Response = PinnedIo;
    type Error = PinnedConnectError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let result = match &mut self.inner {
            PinnedConnectorInner::HttpTunnel(connector) => connector.poll_ready(context),
            PinnedConnectorInner::Socks(connector) => connector.poll_ready(context),
        };
        result.map_err(|error| classify_connect_error(self.kind(), error.as_ref()))
    }

    fn call(&mut self, destination: Uri) -> Self::Future {
        let kind = self.kind();
        let connect_timeout = self.connect_timeout;
        let future = match &mut self.inner {
            PinnedConnectorInner::HttpTunnel(connector) => connector.call(destination),
            PinnedConnectorInner::Socks(connector) => connector.call(destination),
        };
        Box::pin(async move {
            timeout(connect_timeout, future)
                .await
                .map_err(|_| connect_timeout_error(kind))?
                .map_err(|error| classify_connect_error(kind, error.as_ref()))
        })
    }
}

impl PinnedConnector {
    fn kind(&self) -> PinnedConnectorKind {
        match self.inner {
            PinnedConnectorInner::HttpTunnel(_) => PinnedConnectorKind::HttpTunnel,
            PinnedConnectorInner::Socks(_) => PinnedConnectorKind::Socks,
        }
    }
}

#[derive(Clone, Copy)]
enum PinnedConnectorKind {
    HttpTunnel,
    Socks,
}

#[derive(Debug, thiserror::Error)]
#[error("pinned proxy connection failed")]
pub(crate) struct PinnedConnectError {
    pub(crate) stage: TransportErrorStage,
    pub(crate) scope: TransportFailureScope,
    pub(crate) rejected_before_execution: bool,
}

fn classify_connect_error(
    kind: PinnedConnectorKind,
    error: &(dyn StdError + 'static),
) -> PinnedConnectError {
    if error_chain_contains(error, "tunnel error: proxy authorization required") {
        return PinnedConnectError {
            stage: TransportErrorStage::ProxyHandshake,
            scope: TransportFailureScope::Proxy,
            rejected_before_execution: true,
        };
    }
    if error_chain_has_dns_failure(error) {
        return PinnedConnectError {
            stage: TransportErrorStage::Dns,
            scope: TransportFailureScope::EgressPath,
            rejected_before_execution: false,
        };
    }
    if error_chain_has_tls_failure(error) {
        return PinnedConnectError {
            stage: TransportErrorStage::Tls,
            scope: TransportFailureScope::EgressPath,
            rejected_before_execution: false,
        };
    }
    let proxy_failure = match kind {
        PinnedConnectorKind::HttpTunnel => error_chain_starts_with(error, "tunnel error:"),
        PinnedConnectorKind::Socks => error_chain_starts_with(error, "SOCKS error:"),
    };
    if !proxy_failure {
        return PinnedConnectError {
            stage: TransportErrorStage::Tls,
            scope: TransportFailureScope::EgressPath,
            rejected_before_execution: false,
        };
    }
    PinnedConnectError {
        stage: TransportErrorStage::ProxyHandshake,
        scope: match kind {
            PinnedConnectorKind::HttpTunnel | PinnedConnectorKind::Socks => {
                TransportFailureScope::EgressPath
            }
        },
        rejected_before_execution: false,
    }
}

fn connect_timeout_error(kind: PinnedConnectorKind) -> PinnedConnectError {
    match kind {
        PinnedConnectorKind::HttpTunnel | PinnedConnectorKind::Socks => PinnedConnectError {
            stage: TransportErrorStage::ProxyHandshake,
            scope: TransportFailureScope::EgressPath,
            rejected_before_execution: false,
        },
    }
}

fn error_chain_contains(mut error: &(dyn StdError + 'static), message: &str) -> bool {
    loop {
        if error.to_string() == message {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

fn error_chain_starts_with(mut error: &(dyn StdError + 'static), prefix: &str) -> bool {
    loop {
        if error.to_string().starts_with(prefix) {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

fn error_chain_has_dns_failure(mut error: &(dyn StdError + 'static)) -> bool {
    loop {
        if error.downcast_ref::<DnsLookupError>().is_some() {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

fn error_chain_has_tls_failure(mut error: &(dyn StdError + 'static)) -> bool {
    if error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::Other)
    {
        return true;
    }
    loop {
        if error.downcast_ref::<rustls::Error>().is_some() {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
}

type BoxError = Box<dyn StdError + Send + Sync>;

/// Resolves the pinned origin through the shared DNS cache on every connect
/// and races connection handshakes across the resolved addresses, so a dead
/// first address cannot consume the whole connect deadline before fallback.
#[derive(Clone)]
struct ResolvedTarget<C> {
    inner: C,
    host: Arc<str>,
    port: u16,
    secure: bool,
}

impl<C> ResolvedTarget<C> {
    fn new(inner: C, origin: &OriginTarget) -> Self {
        Self {
            inner,
            host: Arc::clone(&origin.host),
            port: origin.port,
            secure: origin.secure,
        }
    }
}

impl<C> Service<Uri> for ResolvedTarget<C>
where
    C: Service<Uri> + Clone + Send + 'static,
    C::Response: Send,
    C::Future: Send,
    C::Error: Into<BoxError>,
{
    type Response = C::Response;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context).map_err(Into::into)
    }

    fn call(&mut self, _destination: Uri) -> Self::Future {
        let inner = self.inner.clone();
        let host = Arc::clone(&self.host);
        let port = self.port;
        let secure = self.secure;
        Box::pin(async move {
            let addresses = shared_dns_cache()
                .resolve(&host)
                .await
                .map_err(BoxError::from)?;
            let mut attempts = FuturesUnordered::new();
            let mut last_error: Option<BoxError> = None;
            let mut tls_error: Option<BoxError> = None;
            for address in addresses.iter() {
                match target_uri(SocketAddr::new(*address, port), secure) {
                    Ok(target) => {
                        let mut connector = inner.clone();
                        attempts.push(async move {
                            poll_fn(|context| connector.poll_ready(context))
                                .await
                                .map_err(Into::<BoxError>::into)?;
                            connector.call(target).await.map_err(Into::into)
                        });
                    }
                    Err(error) => last_error = Some(error),
                }
            }
            while let Some(result) = attempts.next().await {
                match result {
                    Ok(connection) => return Ok(connection),
                    Err(error) => {
                        // A proxy authentication rejection is definitive;
                        // continuing would only bury the most relevant cause.
                        if error_chain_contains(
                            error.as_ref(),
                            "tunnel error: proxy authorization required",
                        ) {
                            return Err(error);
                        }
                        if error_chain_has_tls_failure(error.as_ref()) {
                            tls_error.get_or_insert(error);
                        } else {
                            last_error = Some(error);
                        }
                    }
                }
            }
            if let Some(error) = tls_error {
                return Err(error);
            }
            Err(last_error.expect("resolved address list is never empty"))
        })
    }
}

fn target_uri(address: SocketAddr, secure: bool) -> Result<Uri, BoxError> {
    Uri::builder()
        .scheme(if secure { "https" } else { "http" })
        .authority(address.to_string())
        .path_and_query("/")
        .build()
        .map_err(Into::into)
}

#[cfg(test)]
mod resolved_target_tests {
    use std::{
        future::Future,
        io,
        net::{IpAddr, Ipv4Addr},
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
    };

    use http::Uri;
    use tower_service::Service;

    use super::ResolvedTarget;
    use crate::resolution::{OriginTarget, shared_dns_cache};

    /// Succeeds for any target except the dead first seeded address.
    #[derive(Clone)]
    struct RecordingConnector;

    impl Service<Uri> for RecordingConnector {
        type Response = Uri;
        type Error = io::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Uri, io::Error>> + Send>>;

        fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, destination: Uri) -> Self::Future {
            Box::pin(async move {
                if destination.host() == Some("192.0.2.1") {
                    return Err(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        "dead address",
                    ));
                }
                Ok(destination)
            })
        }
    }

    #[tokio::test]
    async fn pinned_connect_falls_back_across_resolved_addresses() {
        shared_dns_cache().seed(
            "fallback.invalid",
            vec![
                IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
                IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2)),
            ],
        );
        let origin = OriginTarget {
            host: Arc::from("fallback.invalid"),
            port: 443,
            secure: true,
        };
        let mut service = ResolvedTarget::new(RecordingConnector, &origin);

        let connected = service
            .call(Uri::from_static("https://fallback.invalid/"))
            .await
            .expect("second address must connect after the first fails");

        assert_eq!(connected.host(), Some("192.0.2.2"));
        assert_eq!(connected.port_u16(), Some(443));
    }
}
