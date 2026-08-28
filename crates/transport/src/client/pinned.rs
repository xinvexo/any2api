use std::error::Error as StdError;

use any2api_domain::RetrySafety;
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
use tokio::time::Instant;

use super::{
    body_timeout::timeout_body,
    deadline::await_response_headers,
    request_body::{SignaledBody, signaled_body},
};
use crate::{
    api::{
        BoxByteStream, TransportManagerConfig, TransportProxy, TransportRequest, TransportResponse,
    },
    connection::{PinnedConnectError, PinnedConnector},
    error::{TransportError, TransportErrorStage, TransportFailureScope},
    profile::GENERIC_GATEWAY_TRANSPORT_PROFILE as WIRE_PROFILE,
    resolution::OriginTarget,
    response_coding::{decode_response_content, sanitize_request_accept_encoding},
};

pub(crate) struct PinnedClient {
    client: Client<PinnedConnector, SignaledBody>,
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
            proxy_authorization,
        )?;
        let mut builder = Client::builder(TokioExecutor::new());
        builder
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .pool_timer(TokioTimer::new())
            .timer(TokioTimer::new())
            .http2_keep_alive_interval(WIRE_PROFILE.http2_keep_alive_interval())
            .http2_keep_alive_timeout(WIRE_PROFILE.http2_keep_alive_timeout())
            .http2_keep_alive_while_idle(WIRE_PROFILE.http2_keep_alive_while_idle())
            .retry_canceled_requests(false);
        Ok(Self {
            client: builder.build(connector),
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
        sanitize_request_accept_encoding(&mut headers);
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
        let (body, body_sent) = signaled_body(request.body);
        let mut upstream = Request::builder()
            .method(request.method)
            .uri(request.uri)
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
        decode_response_content(TransportResponse {
            status,
            headers,
            body: timeout_body(body, read_timeout, TransportFailureScope::EgressPath),
            read_failure_scope: TransportFailureScope::EgressPath,
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
