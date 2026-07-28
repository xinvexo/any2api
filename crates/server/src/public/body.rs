use std::io::{Cursor, Read};

use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

use super::error::PublicApiError;

const MAX_DECOMPRESSED_PUBLIC_BODY_BYTES: usize =
    any2api_runtime::api::STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES;

/// Buffered public request body whose rejection uses the dialect-aware
/// protocol error envelope instead of axum's plain-text response.
pub(super) struct PublicBody(pub(super) Bytes);

impl FromRequest<AppState> for PublicBody {
    type Rejection = Response;

    async fn from_request(request: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        let uri = request.uri().clone();
        let encoding = content_encoding(request.headers());
        if encoding == ContentEncoding::Invalid
            || (encoding == ContentEncoding::Zstd && !path_supports_zstd(uri.path()))
        {
            return Err(
                PublicApiError::unsupported_content_encoding().into_response_for(state, &uri)
            );
        }
        match Bytes::from_request(request, state).await {
            Ok(bytes) if encoding == ContentEncoding::Zstd => {
                let decoded = tokio::task::spawn_blocking(move || decode_zstd(bytes))
                    .await
                    .map_err(|_| {
                        PublicApiError::unreadable_body().into_response_for(state, &uri)
                    })?;
                match decoded {
                    Ok(bytes) => Ok(Self(bytes)),
                    Err(ZstdBodyError::TooLarge) => {
                        Err(PublicApiError::payload_too_large().into_response_for(state, &uri))
                    }
                    Err(ZstdBodyError::Invalid) => {
                        Err(PublicApiError::unreadable_body().into_response_for(state, &uri))
                    }
                }
            }
            Ok(bytes) => Ok(Self(bytes)),
            Err(rejection) => {
                let error = if rejection.into_response().status() == StatusCode::PAYLOAD_TOO_LARGE {
                    PublicApiError::payload_too_large()
                } else {
                    PublicApiError::unreadable_body()
                };
                Err(error.into_response_for(state, &uri))
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ContentEncoding {
    Identity,
    Zstd,
    Invalid,
}

fn content_encoding(headers: &axum::http::HeaderMap) -> ContentEncoding {
    let mut values = headers.get_all(axum::http::header::CONTENT_ENCODING).iter();
    let Some(value) = values.next() else {
        return ContentEncoding::Identity;
    };
    if values.next().is_some() {
        return ContentEncoding::Invalid;
    }
    match value.to_str().ok().map(str::trim) {
        Some(value) if value.eq_ignore_ascii_case("identity") => ContentEncoding::Identity,
        Some(value) if value.eq_ignore_ascii_case("zstd") => ContentEncoding::Zstd,
        _ => ContentEncoding::Invalid,
    }
}

fn path_supports_zstd(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    path.ends_with("/responses")
        || path.ends_with("/responses/compact")
        || path.ends_with("/chat/completions")
}

fn decode_zstd(bytes: Bytes) -> Result<Bytes, ZstdBodyError> {
    let decoder =
        zstd::stream::read::Decoder::new(Cursor::new(bytes)).map_err(|_| ZstdBodyError::Invalid)?;
    let limit = u64::try_from(MAX_DECOMPRESSED_PUBLIC_BODY_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut limited = decoder.take(limit);
    let mut decoded = Vec::new();
    limited
        .read_to_end(&mut decoded)
        .map_err(|_| ZstdBodyError::Invalid)?;
    if decoded.len() > MAX_DECOMPRESSED_PUBLIC_BODY_BYTES {
        return Err(ZstdBodyError::TooLarge);
    }
    Ok(Bytes::from(decoded))
}

#[derive(Debug)]
enum ZstdBodyError {
    Invalid,
    TooLarge,
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Bytes,
        http::{HeaderMap, HeaderValue, header},
    };

    use super::{
        ContentEncoding, MAX_DECOMPRESSED_PUBLIC_BODY_BYTES, ZstdBodyError, content_encoding,
        decode_zstd, path_supports_zstd,
    };

    #[test]
    fn content_encoding_is_single_and_zstd_is_limited_to_json_openai_routes() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("identity"),
        );
        assert!(matches!(
            content_encoding(&headers),
            ContentEncoding::Identity
        ));
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("zstd"));
        assert!(matches!(content_encoding(&headers), ContentEncoding::Zstd));
        assert!(path_supports_zstd("/v1/responses"));
        assert!(path_supports_zstd("/v1/responses/compact"));
        assert!(!path_supports_zstd("/v1/messages"));
        assert!(!path_supports_zstd("/v1/images/edits"));
        headers.append(header::CONTENT_ENCODING, HeaderValue::from_static("zstd"));
        assert!(matches!(
            content_encoding(&headers),
            ContentEncoding::Invalid
        ));
        headers.insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        assert!(matches!(
            content_encoding(&headers),
            ContentEncoding::Invalid
        ));
    }

    #[test]
    fn valid_zstd_body_is_decoded_and_damaged_body_is_rejected() {
        let encoded =
            zstd::stream::encode_all(&b"{\"model\":\"gpt\"}"[..], 3).expect("encode zstd");
        assert_eq!(
            decode_zstd(Bytes::from(encoded)).expect("decode zstd"),
            Bytes::from_static(b"{\"model\":\"gpt\"}")
        );
        assert!(decode_zstd(Bytes::from_static(b"not-zstd")).is_err());
    }

    #[test]
    fn zstd_body_is_rejected_when_decompressed_output_exceeds_the_public_limit() {
        let oversized = vec![b'a'; MAX_DECOMPRESSED_PUBLIC_BODY_BYTES + 1];
        let encoded = zstd::stream::encode_all(oversized.as_slice(), 1).expect("encode zstd");
        assert!(matches!(
            decode_zstd(Bytes::from(encoded)),
            Err(ZstdBodyError::TooLarge)
        ));
    }
}
