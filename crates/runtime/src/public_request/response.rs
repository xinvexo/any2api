use any2api_domain::{ErrorClass, PublicError, PublicErrorCode};
use any2api_transport::api::BoxByteStream;
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use http::{HeaderMap, HeaderName, header};

pub(super) const MAX_UPSTREAM_JSON_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_CLASSIFIED_ERROR_BYTES: usize = 64 * 1024;

pub(super) async fn collect_body(mut body: BoxByteStream) -> Result<Bytes, PublicError> {
    let mut collected = BytesMut::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|_| {
            public_error(PublicErrorCode::UpstreamError, "upstream response failed")
        })?;
        if collected.len().saturating_add(chunk.len()) > MAX_UPSTREAM_JSON_BYTES {
            return Err(public_error(
                PublicErrorCode::UpstreamError,
                "upstream response exceeded the configured limit",
            ));
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}

pub(super) fn restore_public_model(
    body: &mut Bytes,
    public_model: &str,
) -> Result<(), PublicError> {
    let mut value: serde_json::Value = serde_json::from_slice(body).map_err(|_| {
        public_error(
            PublicErrorCode::UpstreamError,
            "upstream response was not valid JSON",
        )
    })?;
    if let Some(object) = value.as_object_mut()
        && object.contains_key("model")
    {
        object.insert(
            "model".into(),
            serde_json::Value::String(public_model.to_owned()),
        );
        *body = Bytes::from(serde_json::to_vec(&value).map_err(|_| internal_error())?);
    }
    Ok(())
}

pub(super) fn sanitize_response_headers(headers: &mut HeaderMap) {
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
    headers.remove("content-encoding");
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

pub(super) fn classified_error(class: ErrorClass) -> PublicError {
    if class == ErrorClass::OperationUnavailable {
        return public_error(
            PublicErrorCode::UpstreamNotFound,
            "upstream operation is unavailable",
        );
    }
    let message = match class {
        ErrorClass::Authentication => "upstream authentication failed",
        ErrorClass::PermissionDenied => "upstream permission was denied",
        ErrorClass::RateLimited => "upstream rate limit was reached",
        ErrorClass::ModelUnavailable => "upstream model is unavailable",
        _ => "upstream service returned an error",
    };
    public_error(PublicErrorCode::UpstreamError, message)
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

pub(super) fn public_error(code: PublicErrorCode, message: &'static str) -> PublicError {
    PublicError::new(code, message)
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue, header};

    use super::sanitize_response_headers;

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
}
