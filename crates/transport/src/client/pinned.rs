use std::{error::Error as StdError, net::SocketAddr};

use any2api_domain::{ProxyKind, RetrySafety};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use http::{
    HeaderValue, Request,
    header::{HOST, PROXY_AUTHORIZATION},
};
use http_body_util::BodyExt;
use hyper_util::{
    client::legacy::Client,
    rt::{TokioExecutor, TokioTimer},
};
use rustls::ClientConfig;
use tokio::time::{Instant, timeout_at};

use super::{
    body_timeout::timeout_body,
    construction::{HTTP2_KEEP_ALIVE_INTERVAL, HTTP2_KEEP_ALIVE_TIMEOUT},
    deadline::await_response_headers,
    request_body::{SignaledBody, signaled_body},
};
use crate::{
    api::{
        BoxByteStream, TransportManagerConfig, TransportProxy, TransportRequest, TransportResponse,
    },
    connection::{PinnedConnectError, PinnedConnector},
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    resolution::{OriginTarget, shared_dns_cache},
};

pub(crate) struct PinnedClient {
    client: Client<PinnedConnector, SignaledBody>,
    origin: OriginTarget,
    forward_proxy: bool,
    proxy_authorization: Option<HeaderValue>,
}

impl PinnedClient {
    pub(crate) fn build(
        config: TransportManagerConfig,
        tls_config: ClientConfig,
        proxy: TransportProxy<'_>,
        origin: &OriginTarget,
    ) -> Result<Self, TransportError> {
        let proxy_authorization = basic_proxy_authorization(proxy)?;
        let connector = PinnedConnector::build(
            config.connect_timeout,
            tls_config,
            proxy,
            origin,
            proxy_authorization.clone(),
        )?;
        let mut builder = Client::builder(TokioExecutor::new());
        builder
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .pool_timer(TokioTimer::new())
            .timer(TokioTimer::new())
            .http2_keep_alive_interval(HTTP2_KEEP_ALIVE_INTERVAL)
            .http2_keep_alive_timeout(HTTP2_KEEP_ALIVE_TIMEOUT)
            .retry_canceled_requests(false);
        Ok(Self {
            client: builder.build(connector),
            origin: origin.clone(),
            forward_proxy: proxy.profile().kind() == ProxyKind::Http && !origin.secure,
            proxy_authorization,
        })
    }

    pub(crate) async fn execute(
        &self,
        request: TransportRequest,
        connect_deadline: Instant,
        connect_timeout_error: TransportError,
    ) -> Result<TransportResponse, TransportError> {
        let read_timeout = request.read_timeout;
        let mut headers = request.headers;
        headers.remove(PROXY_AUTHORIZATION);
        let authority = request.uri.authority().ok_or_else(|| {
            TransportError::new(
                TransportErrorStage::WriteRequest,
                TransportFailureScope::Unattributed,
                RetrySafety::DefinitelyNotSent,
                "upstream URI has no authority",
            )
        })?;
        headers.insert(
            HOST,
            HeaderValue::from_str(authority.as_str()).map_err(|_| {
                TransportError::new(
                    TransportErrorStage::WriteRequest,
                    TransportFailureScope::Unattributed,
                    RetrySafety::DefinitelyNotSent,
                    "upstream authority cannot be encoded as a Host header",
                )
            })?,
        );
        if self.forward_proxy
            && let Some(value) = &self.proxy_authorization
        {
            headers.insert(PROXY_AUTHORIZATION, value.clone());
        }

        let (body, body_sent) = signaled_body(request.body);
        let uri = if self.forward_proxy {
            let target = self
                .forward_target(connect_deadline, &connect_timeout_error)
                .await?;
            rewrite_uri(&request.uri, target)
        } else {
            request.uri.clone()
        };
        let mut upstream = Request::builder()
            .method(request.method)
            .uri(uri)
            .body(body)
            .map_err(|_| {
                TransportError::new(
                    TransportErrorStage::WriteRequest,
                    TransportFailureScope::Unattributed,
                    RetrySafety::DefinitelyNotSent,
                    "failed to build pinned upstream request",
                )
            })?;
        *upstream.headers_mut() = headers;

        let response = await_response_headers(
            self.client.request(upstream),
            body_sent,
            connect_deadline,
            read_timeout,
            map_send_error,
            connect_timeout_error,
            await_headers_timeout(),
        )
        .await?;
        let status = response.status();
        if self.forward_proxy && status == http::StatusCode::PROXY_AUTHENTICATION_REQUIRED {
            return Err(TransportError::new(
                TransportErrorStage::ProxyHandshake,
                TransportFailureScope::Proxy,
                RetrySafety::RejectedBeforeExecution,
                "configured proxy authentication was rejected",
            ));
        }
        let headers = response.headers().clone();
        let body: BoxByteStream = Box::pin(response.into_body().into_data_stream().map(|result| {
            result.map_err(|_| {
                TransportError::new(
                    TransportErrorStage::ReadBody,
                    TransportFailureScope::EgressPath,
                    RetrySafety::Ambiguous,
                    "upstream response body read failed",
                )
            })
        }));
        Ok(TransportResponse {
            status,
            headers,
            body: timeout_body(body, read_timeout, TransportFailureScope::EgressPath),
            read_failure_scope: TransportFailureScope::EgressPath,
        })
    }

    /// Resolves the pinned target for HTTP forward-proxy URI rewriting on
    /// every request, so a rotated or revived DNS record is picked up as soon
    /// as the shared cache entry expires.
    async fn forward_target(
        &self,
        connect_deadline: Instant,
        connect_timeout_error: &TransportError,
    ) -> Result<SocketAddr, TransportError> {
        let addresses = timeout_at(
            connect_deadline,
            shared_dns_cache().resolve(&self.origin.host),
        )
        .await
        .map_err(|_| connect_timeout_error.clone())?
        .map_err(|error| {
            TransportError::new(
                TransportErrorStage::Dns,
                TransportFailureScope::EgressPath,
                RetrySafety::DefinitelyNotSent,
                error.to_string(),
            )
        })?;
        let address = *addresses
            .first()
            .expect("resolved address list is never empty");
        Ok(SocketAddr::new(address, self.origin.port))
    }
}

fn basic_proxy_authorization(
    proxy: TransportProxy<'_>,
) -> Result<Option<HeaderValue>, TransportError> {
    let Some(credentials) = proxy.credentials() else {
        return Ok(None);
    };
    let encoded = STANDARD.encode(format!(
        "{}:{}",
        credentials.username(),
        credentials.password()
    ));
    let mut value = HeaderValue::from_str(&format!("Basic {encoded}")).map_err(|_| {
        TransportError::configuration(
            TransportErrorStage::ProxyHandshake,
            TransportFailureScope::Proxy,
            "proxy authentication is invalid",
        )
    })?;
    value.set_sensitive(true);
    Ok(Some(value))
}

fn rewrite_uri(original: &http::Uri, target: SocketAddr) -> http::Uri {
    http::Uri::builder()
        .scheme(
            original
                .scheme()
                .cloned()
                .expect("validated URI has a scheme"),
        )
        .authority(target.to_string())
        .path_and_query(
            original
                .path_and_query()
                .cloned()
                .unwrap_or_else(|| http::uri::PathAndQuery::from_static("/")),
        )
        .build()
        .expect("validated pinned URI components are valid")
}

fn map_send_error(error: hyper_util::client::legacy::Error) -> TransportError {
    if error.is_connect()
        && let Some(connect) = find_source::<PinnedConnectError>(&error)
    {
        return TransportError::new(
            connect.stage,
            connect.scope,
            if connect.rejected_before_execution {
                RetrySafety::RejectedBeforeExecution
            } else {
                RetrySafety::DefinitelyNotSent
            },
            match connect.scope {
                TransportFailureScope::Endpoint => "pinned upstream connection failed",
                TransportFailureScope::Proxy => "configured proxy connection failed",
                TransportFailureScope::EgressPath | TransportFailureScope::Unattributed => {
                    "configured proxy connection failed"
                }
            },
        );
    }
    if error.is_connect() {
        return TransportError::new(
            TransportErrorStage::ProxyHandshake,
            TransportFailureScope::EgressPath,
            RetrySafety::DefinitelyNotSent,
            "pinned proxy connection failed",
        );
    }
    TransportError::new(
        TransportErrorStage::AwaitHeaders,
        TransportFailureScope::EgressPath,
        RetrySafety::Ambiguous,
        "upstream request failed before response headers",
    )
}

fn await_headers_timeout() -> TransportError {
    TransportError::new(
        TransportErrorStage::AwaitHeaders,
        TransportFailureScope::EgressPath,
        RetrySafety::Ambiguous,
        "upstream response headers timed out",
    )
}

fn find_source<'a, T: StdError + 'static>(
    mut error: &'a (dyn StdError + 'static),
) -> Option<&'a T> {
    loop {
        if let Some(found) = error.downcast_ref::<T>() {
            return Some(found);
        }
        error = error.source()?;
    }
}
