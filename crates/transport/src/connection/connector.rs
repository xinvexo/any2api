use std::{
    error::Error as StdError,
    fmt,
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::ProxyKind;
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

type HttpForwardConnector = HttpsConnector<ProxyTcpConnector>;
type HttpTunnelConnector = HttpsConnector<ResolvedTarget<Tunnel<ProxyTcpConnector>>>;
type SocksConnector = HttpsConnector<ResolvedTarget<SocksV5<ProxyTcpConnector>>>;

pub(crate) type PinnedIo = MaybeHttpsStream<ProxyTcpStream>;

#[derive(Clone)]
pub(crate) struct PinnedConnector {
    inner: PinnedConnectorInner,
    connect_timeout: Duration,
}

#[derive(Clone)]
enum PinnedConnectorInner {
    HttpForward(HttpForwardConnector),
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
                TransportFailureScope::Endpoint,
                "upstream TLS server name is invalid",
            )
        })?;

        match (profile.kind(), origin.secure) {
            (ProxyKind::Http, false) => {
                let tcp =
                    ProxyTcpConnector::new(address.host(), address.port(), connect_timeout, true);
                Ok(Self {
                    inner: PinnedConnectorInner::HttpForward(wrap_tls(
                        tcp,
                        tls_config,
                        server_name,
                    )),
                    connect_timeout,
                })
            }
            (ProxyKind::Http, true) => {
                let tcp =
                    ProxyTcpConnector::new(address.host(), address.port(), connect_timeout, false);
                let mut tunnel = Tunnel::new(proxy_uri(address.host(), address.port())?, tcp);
                if let Some(value) = proxy_authorization {
                    tunnel = tunnel.with_auth(value);
                }
                Ok(Self {
                    inner: PinnedConnectorInner::HttpTunnel(wrap_tls(
                        ResolvedTarget::new(tunnel, origin),
                        tls_config,
                        server_name,
                    )),
                    connect_timeout,
                })
            }
            (ProxyKind::Socks5, _) => {
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
                    inner: PinnedConnectorInner::Socks(wrap_tls(
                        ResolvedTarget::new(socks, origin),
                        tls_config,
                        server_name,
                    )),
                    connect_timeout,
                })
            }
            (ProxyKind::Direct, _) => Err(TransportError::configuration(
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
                PinnedConnectorInner::HttpForward(_) => "http_forward",
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
            PinnedConnectorInner::HttpForward(connector) => connector.poll_ready(context),
            PinnedConnectorInner::HttpTunnel(connector) => connector.poll_ready(context),
            PinnedConnectorInner::Socks(connector) => connector.poll_ready(context),
        };
        result.map_err(|error| classify_connect_error(self.kind(), error.as_ref()))
    }

    fn call(&mut self, destination: Uri) -> Self::Future {
        let kind = self.kind();
        let connect_timeout = self.connect_timeout;
        let future = match &mut self.inner {
            PinnedConnectorInner::HttpForward(connector) => connector.call(destination),
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
            PinnedConnectorInner::HttpForward(_) => PinnedConnectorKind::HttpForward,
            PinnedConnectorInner::HttpTunnel(_) => PinnedConnectorKind::HttpTunnel,
            PinnedConnectorInner::Socks(_) => PinnedConnectorKind::Socks,
        }
    }
}

#[derive(Clone, Copy)]
enum PinnedConnectorKind {
    HttpForward,
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
            scope: TransportFailureScope::Endpoint,
            rejected_before_execution: false,
        };
    }
    let proxy_failure = match kind {
        PinnedConnectorKind::HttpForward => true,
        PinnedConnectorKind::HttpTunnel => error_chain_starts_with(error, "tunnel error:"),
        PinnedConnectorKind::Socks => error_chain_starts_with(error, "SOCKS error:"),
    };
    if !proxy_failure {
        return PinnedConnectError {
            stage: TransportErrorStage::Tls,
            scope: TransportFailureScope::Endpoint,
            rejected_before_execution: false,
        };
    }
    PinnedConnectError {
        stage: TransportErrorStage::ProxyHandshake,
        scope: TransportFailureScope::Proxy,
        rejected_before_execution: false,
    }
}

fn connect_timeout_error(kind: PinnedConnectorKind) -> PinnedConnectError {
    match kind {
        PinnedConnectorKind::HttpForward => PinnedConnectError {
            stage: TransportErrorStage::ProxyHandshake,
            scope: TransportFailureScope::Proxy,
            rejected_before_execution: false,
        },
        PinnedConnectorKind::HttpTunnel | PinnedConnectorKind::Socks => PinnedConnectError {
            stage: TransportErrorStage::ProxyHandshake,
            scope: TransportFailureScope::Unattributed,
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

type BoxError = Box<dyn StdError + Send + Sync>;

/// Resolves the pinned origin through the shared DNS cache on every connect
/// and falls back across the resolved addresses in order, so a dead first
/// address never permanently black-holes the origin.
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
        let mut inner = self.inner.clone();
        let host = Arc::clone(&self.host);
        let port = self.port;
        let secure = self.secure;
        Box::pin(async move {
            let addresses = shared_dns_cache()
                .resolve(&host)
                .await
                .map_err(BoxError::from)?;
            let mut last_error: Option<BoxError> = None;
            for address in addresses.iter() {
                match target_uri(SocketAddr::new(*address, port), secure) {
                    Ok(target) => match inner.call(target).await {
                        Ok(connection) => return Ok(connection),
                        Err(error) => {
                            let error = error.into();
                            // A proxy authentication rejection is definitive;
                            // trying further target addresses would only bury
                            // the most relevant cause.
                            if error_chain_contains(
                                error.as_ref(),
                                "tunnel error: proxy authorization required",
                            ) {
                                return Err(error);
                            }
                            last_error = Some(error);
                        }
                    },
                    Err(error) => last_error = Some(error),
                }
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
