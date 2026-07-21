use std::{error::Error as StdError, net::SocketAddr};

use any2api_domain::{ProxyKind, RetrySafety};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use http::{
    HeaderValue, Request,
    header::{HOST, PROXY_AUTHORIZATION},
};
use http_body_util::BodyExt;
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use rustls::pki_types::CertificateDer;
use tokio::time::timeout;

use crate::{
    api::{
        BoxByteStream, TransportManagerConfig, TransportProxy, TransportRequest, TransportResponse,
    },
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    origin_resolution::ResolvedOrigin,
    pinned_connector::{PinnedConnectError, PinnedConnector},
    request_body::{SignaledBody, signaled_body},
};

pub(crate) struct PinnedClient {
    client: Client<PinnedConnector, SignaledBody>,
    target: SocketAddr,
    forward_proxy: bool,
    proxy_authorization: Option<HeaderValue>,
}

impl PinnedClient {
    pub(crate) fn build(
        config: TransportManagerConfig,
        extra_roots: &[CertificateDer<'static>],
        proxy: TransportProxy<'_>,
        origin: &ResolvedOrigin,
    ) -> Result<Self, TransportError> {
        let target = *origin.addresses.first().ok_or_else(|| {
            TransportError::configuration("resolved upstream address list is empty")
        })?;
        let proxy_authorization = basic_proxy_authorization(proxy)?;
        let connector = PinnedConnector::build(
            config.connect_timeout,
            extra_roots,
            proxy,
            origin,
            proxy_authorization.clone(),
        )?;
        let mut builder = Client::builder(TokioExecutor::new());
        builder
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .retry_canceled_requests(false);
        Ok(Self {
            client: builder.build(connector),
            target,
            forward_proxy: proxy.profile().kind() == ProxyKind::Http && !origin.secure,
            proxy_authorization,
        })
    }

    pub(crate) async fn execute(
        &self,
        request: TransportRequest,
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
            rewrite_uri(&request.uri, self.target)
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

        let send = self.client.request(upstream);
        tokio::pin!(send);
        let response = tokio::select! {
            biased;
            result = &mut send => result.map_err(map_send_error),
            signal = body_sent => {
                if signal.is_err() {
                    (&mut send).await.map_err(map_send_error)
                } else {
                    timeout(read_timeout, &mut send)
                        .await
                        .map_err(|_| await_headers_timeout())?
                        .map_err(map_send_error)
                }
            }
        }?;
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
                    TransportFailureScope::Unattributed,
                    RetrySafety::Ambiguous,
                    "upstream response body read failed",
                )
            })
        }));
        Ok(TransportResponse {
            status,
            headers,
            body,
            read_failure_scope: TransportFailureScope::Unattributed,
        })
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
    let mut value = HeaderValue::from_str(&format!("Basic {encoded}"))
        .map_err(|_| TransportError::configuration("proxy authentication is invalid"))?;
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
                TransportFailureScope::Proxy | TransportFailureScope::Unattributed => {
                    "configured proxy connection failed"
                }
            },
        );
    }
    if error.is_connect() {
        return TransportError::new(
            TransportErrorStage::ProxyHandshake,
            TransportFailureScope::Unattributed,
            RetrySafety::DefinitelyNotSent,
            "pinned proxy connection failed",
        );
    }
    TransportError::new(
        TransportErrorStage::AwaitHeaders,
        TransportFailureScope::Unattributed,
        RetrySafety::Ambiguous,
        "upstream request failed before response headers",
    )
}

fn await_headers_timeout() -> TransportError {
    TransportError::new(
        TransportErrorStage::AwaitHeaders,
        TransportFailureScope::Unattributed,
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
