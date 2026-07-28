use std::time::Duration;

use any2api_domain::{ErrorClass, PublicError, PublicErrorCode, UpstreamError};
use any2api_protocol::api::EgressResponse;
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use http::{HeaderMap, HeaderName, StatusCode, header};
use tokio::time::{Instant, timeout, timeout_at};

pub(super) const MAX_UPSTREAM_ERROR_BODY_BYTES: usize = 64 * 1024;

pub(super) enum FinalFailure {
    Local {
        error: PublicError,
    },
    Upstream {
        response: EgressResponse,
        error_class: ErrorClass,
        error_message: Option<String>,
    },
}

impl FinalFailure {
    pub(super) fn upstream(
        headers: HeaderMap,
        status: StatusCode,
        body: Bytes,
        upstream: &UpstreamError,
    ) -> Self {
        Self::Upstream {
            response: EgressResponse {
                status,
                headers,
                body,
            },
            error_class: upstream.classification().kind().error_class(),
            error_message: upstream.official_message().map(ToOwned::to_owned),
        }
    }
}

impl From<PublicError> for FinalFailure {
    fn from(error: PublicError) -> Self {
        Self::Local { error }
    }
}

#[derive(Debug)]
pub(super) enum CollectBodyError {
    Transport(TransportError),
    Public(PublicError),
}

pub(super) async fn collect_body(
    mut body: BoxByteStream,
    read_timeout: Duration,
    max_bytes: usize,
    failure_scope: TransportFailureScope,
) -> Result<Bytes, CollectBodyError> {
    let mut collected = BytesMut::new();
    loop {
        let next = timeout(read_timeout, body.next()).await.map_err(|_| {
            CollectBodyError::Transport(TransportError::new(
                TransportErrorStage::ReadBody,
                failure_scope,
                any2api_domain::RetrySafety::Ambiguous,
                "upstream response body read timed out",
            ))
        })?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(CollectBodyError::Transport)?;
        if collected.len().saturating_add(chunk.len()) > max_bytes {
            return Err(CollectBodyError::Public(public_error(
                PublicErrorCode::UpstreamError,
                "upstream response exceeded the configured limit",
            )));
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}

pub(super) async fn collect_error_body(
    mut body: BoxByteStream,
    read_timeout: Duration,
    attempt_deadline: Instant,
) -> Option<Bytes> {
    let mut collected = BytesMut::new();
    loop {
        let idle_deadline = Instant::now() + read_timeout;
        let next = timeout_at(idle_deadline.min(attempt_deadline), body.next())
            .await
            .ok()?;
        let Some(chunk) = next else {
            return Some(collected.freeze());
        };
        let chunk = chunk.ok()?;
        if collected.len().saturating_add(chunk.len()) > MAX_UPSTREAM_ERROR_BODY_BYTES {
            return None;
        }
        collected.extend_from_slice(&chunk);
    }
}

pub(super) fn sanitize_response_headers(headers: &mut HeaderMap) {
    sanitize_response_headers_inner(headers, false);
}

pub(super) fn sanitize_upstream_error_response_headers(headers: &mut HeaderMap, body: &Bytes) {
    sanitize_response_headers_inner(headers, !body.is_empty());
}

fn sanitize_response_headers_inner(headers: &mut HeaderMap, preserve_content_encoding: bool) {
    let nominated = headers
        .get_all(header::CONNECTION)
        .iter()
        .flat_map(|value| value.as_bytes().split(|byte| *byte == b','))
        .filter_map(|name| HeaderName::from_bytes(trim_ows(name)).ok())
        .collect::<Vec<_>>();
    for name in nominated {
        headers.remove(name);
    }

    for name in [
        header::CONNECTION,
        header::CONTENT_LENGTH,
        header::PROXY_AUTHENTICATE,
        header::PROXY_AUTHORIZATION,
        header::TE,
        header::TRAILER,
        header::TRANSFER_ENCODING,
        header::UPGRADE,
        header::AUTHORIZATION,
        header::COOKIE,
        header::CONTENT_RANGE,
        header::ETAG,
        header::SET_COOKIE,
    ] {
        headers.remove(name);
    }
    headers.remove("keep-alive");
    headers.remove("content-md5");
    if !preserve_content_encoding {
        headers.remove("content-encoding");
    }
    headers.remove("digest");
    headers.remove("x-api-key");
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

pub(super) fn invalid_request(message: &'static str) -> PublicError {
    public_error(PublicErrorCode::InvalidRequest, message)
}

pub(super) fn internal_error() -> PublicError {
    public_error(
        PublicErrorCode::InternalError,
        "internal request planning failed",
    )
}

pub(super) const fn transport_error_diagnostic(error: &TransportError) -> &'static str {
    match error.stage {
        TransportErrorStage::Dns => "upstream DNS resolution failed",
        TransportErrorStage::Tcp => "upstream TCP connection failed",
        TransportErrorStage::ProxyHandshake => "upstream proxy handshake failed",
        TransportErrorStage::Tls => "upstream TLS handshake failed",
        TransportErrorStage::WriteRequest => "upstream request write failed",
        TransportErrorStage::AwaitHeaders => "upstream response headers failed",
        TransportErrorStage::ReadBody => "upstream response body failed",
    }
}

pub(super) fn public_error(code: PublicErrorCode, message: &'static str) -> PublicError {
    PublicError::new(code, message)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use any2api_domain::RetrySafety;
    use any2api_transport::api::{
        BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
    };
    use bytes::Bytes;
    use futures_util::{StreamExt, stream};
    use http::{HeaderMap, HeaderValue, header};

    use super::{
        CollectBodyError, MAX_UPSTREAM_ERROR_BODY_BYTES, collect_body, collect_error_body,
        sanitize_response_headers, sanitize_upstream_error_response_headers,
    };

    #[tokio::test]
    async fn error_body_is_available_only_when_complete_and_within_its_limit() {
        let complete: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from_static(b"complete"))]));
        assert_eq!(
            collect_error_body(
                complete,
                Duration::from_secs(1),
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await,
            Some(Bytes::from_static(b"complete"))
        );

        let oversized: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from(vec![
            b'x';
            MAX_UPSTREAM_ERROR_BODY_BYTES
                + 1
        ]))]));
        assert_eq!(
            collect_error_body(
                oversized,
                Duration::from_secs(1),
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await,
            None
        );

        let read_error = TransportError::new(
            TransportErrorStage::ReadBody,
            TransportFailureScope::Endpoint,
            RetrySafety::Ambiguous,
            "truncated error body",
        );
        let incomplete: BoxByteStream = Box::pin(stream::iter([
            Ok(Bytes::from_static(b"partial")),
            Err(read_error),
        ]));
        assert_eq!(
            collect_error_body(
                incomplete,
                Duration::from_secs(1),
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await,
            None
        );
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_deadline_discards_an_unfinished_error_body() {
        let body: BoxByteStream =
            Box::pin(stream::iter([Ok(Bytes::from_static(b"partial"))]).chain(stream::pending()));

        let collected = collect_error_body(
            body,
            Duration::from_secs(1),
            tokio::time::Instant::now() + Duration::from_millis(25),
        )
        .await;

        assert_eq!(collected, None);
    }

    #[tokio::test]
    async fn collect_body_preserves_transport_failure_metadata() {
        let expected = TransportError::new(
            TransportErrorStage::ReadBody,
            TransportFailureScope::Proxy,
            RetrySafety::DefinitelyNotSent,
            "proxy response body failed",
        );
        let body: BoxByteStream = Box::pin(stream::iter([Err(expected.clone())]));

        let error = collect_body(
            body,
            Duration::from_secs(1),
            1024,
            TransportFailureScope::Proxy,
        )
        .await
        .expect_err("body must fail");

        match error {
            CollectBodyError::Transport(error) => assert_eq!(error, expected),
            CollectBodyError::Public(_) => panic!("transport error must keep its metadata"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn collect_body_times_out_while_waiting_for_the_next_chunk() {
        let body: BoxByteStream =
            Box::pin(stream::iter([Ok(Bytes::from_static(b"partial"))]).chain(stream::pending()));

        let error = collect_body(
            body,
            Duration::from_millis(25),
            1024,
            TransportFailureScope::Endpoint,
        )
        .await
        .expect_err("stalled body must time out");

        match error {
            CollectBodyError::Transport(error) => {
                assert_eq!(error.stage, TransportErrorStage::ReadBody);
                assert_eq!(error.failure_scope, TransportFailureScope::Endpoint);
                assert_eq!(error.retry_safety, RetrySafety::Ambiguous);
            }
            CollectBodyError::Public(_) => panic!("timeout must remain a transport failure"),
        }
    }

    #[test]
    fn response_headers_remove_sensitive_and_nominated_hop_by_hop_fields() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static("secret"));
        headers.insert("x-api-key", HeaderValue::from_static("secret"));
        headers.insert(header::COOKIE, HeaderValue::from_static("secret=1"));
        headers.insert(header::SET_COOKIE, HeaderValue::from_static("secret=1"));
        headers.insert(header::ETAG, HeaderValue::from_static("\"upstream-body\""));
        headers.insert("digest", HeaderValue::from_static("sha-256=stale"));
        headers.insert("content-encoding", HeaderValue::from_static("gzip"));
        headers.insert(
            header::CONNECTION,
            HeaderValue::from_bytes(b"x-private-hop,\x80").expect("opaque connection value"),
        );
        headers.insert("x-private-hop", HeaderValue::from_static("secret"));
        headers.insert("x-request-id", HeaderValue::from_static("request-1"));

        sanitize_response_headers(&mut headers);

        for name in [
            header::AUTHORIZATION,
            header::COOKIE,
            header::SET_COOKIE,
            header::CONNECTION,
            header::ETAG,
        ] {
            assert!(headers.get(name).is_none());
        }
        assert!(headers.get("digest").is_none());
        assert!(headers.get("content-encoding").is_none());
        assert!(headers.get("x-api-key").is_none());
        assert!(headers.get("x-private-hop").is_none());
        assert_eq!(headers["x-request-id"], "request-1");
    }

    #[test]
    fn transparent_error_headers_keep_content_encoding_only_with_a_body() {
        let mut complete = HeaderMap::new();
        complete.insert("content-encoding", HeaderValue::from_static("gzip"));
        sanitize_upstream_error_response_headers(&mut complete, &Bytes::from_static(b"compressed"));
        assert_eq!(complete["content-encoding"], "gzip");

        let mut empty = HeaderMap::new();
        empty.insert("content-encoding", HeaderValue::from_static("gzip"));
        sanitize_upstream_error_response_headers(&mut empty, &Bytes::new());
        assert!(!empty.contains_key("content-encoding"));
    }
}
