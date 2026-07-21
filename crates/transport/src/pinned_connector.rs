use std::{
    error::Error as StdError,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use any2api_domain::ProxyKind;
use http::{HeaderValue, Uri};
use hyper_rustls::{HttpsConnector, MaybeHttpsStream};
use hyper_util::client::legacy::connect::proxy::{SocksV5, Tunnel};
use rustls::pki_types::{CertificateDer, ServerName};
use tower_service::Service;

use crate::{
    api::TransportProxy,
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    origin_resolution::ResolvedOrigin,
    pinned_tls::{build_tls_config, wrap_tls},
    proxy_tcp_connector::{ProxyTcpConnector, ProxyTcpStream, proxy_uri},
};

type HttpForwardConnector = HttpsConnector<ProxyTcpConnector>;
type HttpTunnelConnector = HttpsConnector<PinnedDestination<Tunnel<ProxyTcpConnector>>>;
type SocksConnector = HttpsConnector<PinnedDestination<SocksV5<ProxyTcpConnector>>>;

pub(crate) type PinnedIo = MaybeHttpsStream<ProxyTcpStream>;

#[derive(Clone)]
pub(crate) struct PinnedConnector {
    inner: PinnedConnectorInner,
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
        extra_roots: &[CertificateDer<'static>],
        proxy: TransportProxy<'_>,
        origin: &ResolvedOrigin,
        proxy_authorization: Option<HeaderValue>,
    ) -> Result<Self, TransportError> {
        let profile = proxy.profile();
        let target = target_uri(origin)?;
        let address = profile.address().ok_or_else(|| {
            TransportError::configuration("configured proxy has no network address")
        })?;
        let server_name = ServerName::try_from(origin.host.to_string())
            .map_err(|_| TransportError::configuration("upstream TLS server name is invalid"))?;
        let tls_config = build_tls_config(extra_roots)?;

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
                        PinnedDestination::new(tunnel, target),
                        tls_config,
                        server_name,
                    )),
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
                        PinnedDestination::new(socks, target),
                        tls_config,
                        server_name,
                    )),
                })
            }
            (ProxyKind::Direct, _) => Err(TransportError::configuration(
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
        let future = match &mut self.inner {
            PinnedConnectorInner::HttpForward(connector) => connector.call(destination),
            PinnedConnectorInner::HttpTunnel(connector) => connector.call(destination),
            PinnedConnectorInner::Socks(connector) => connector.call(destination),
        };
        Box::pin(async move {
            future
                .await
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

fn target_uri(origin: &ResolvedOrigin) -> Result<Uri, TransportError> {
    let target = origin
        .addresses
        .first()
        .ok_or_else(|| TransportError::configuration("resolved upstream address list is empty"))?;
    Uri::builder()
        .scheme(if origin.secure { "https" } else { "http" })
        .authority(target.to_string())
        .path_and_query("/")
        .build()
        .map_err(|_| TransportError::configuration("resolved upstream address is invalid"))
}

#[derive(Clone)]
struct PinnedDestination<C> {
    inner: C,
    target: Uri,
}

impl<C> PinnedDestination<C> {
    fn new(inner: C, target: Uri) -> Self {
        Self { inner, target }
    }
}

impl<C> Service<Uri> for PinnedDestination<C>
where
    C: Service<Uri>,
{
    type Response = C::Response;
    type Error = C::Error;
    type Future = C::Future;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, _destination: Uri) -> Self::Future {
        self.inner.call(self.target.clone())
    }
}
