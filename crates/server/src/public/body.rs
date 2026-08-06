mod collector;

use any2api_domain::ProtocolOperation;
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::HeaderMap,
    response::Response,
};

use crate::{state::AppState, zstd_decode::ZstdDecodeError};

use super::error::PublicApiError;
use collector::{BodyCollectionError, collect_body};

/// Buffered public request body plus the request headers taken by ownership,
/// so handlers avoid axum's `HeaderMap` extractor clone. Rejections use the
/// dialect-aware protocol error envelope instead of axum's plain-text
/// response.
pub(super) struct PublicBody {
    pub(super) headers: HeaderMap,
    pub(super) body: Bytes,
}

impl FromRequest<AppState> for PublicBody {
    type Rejection = Response;

    async fn from_request(request: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        let (parts, body) = request.into_parts();
        let uri = parts.uri;
        let headers = parts.headers;
        let encoding = content_encoding(&headers);
        if encoding == ContentEncoding::Invalid
            || (encoding == ContentEncoding::Zstd && !path_supports_zstd(uri.path()))
        {
            return Err(
                PublicApiError::unsupported_content_encoding().into_response_for(state, &uri)
            );
        }
        let operation = operation_for_path(uri.path())
            .ok_or_else(|| PublicApiError::unreadable_body().into_response_for(state, &uri))?;
        // This collector is the public routes' sole body-size enforcement point.
        // PublicBody consumes the raw Body, so Axum's DefaultBodyLimit is not consulted.
        let bytes = collect_body(body, any2api_runtime::api::request_body_limit(operation))
            .await
            .map_err(|error| body_collection_rejection(error, state, &uri))?;
        let body = match encoding {
            ContentEncoding::Zstd => match state.zstd_decoder().decode(bytes).await {
                Ok(bytes) => bytes,
                Err(ZstdDecodeError::TooLarge) => {
                    return Err(PublicApiError::payload_too_large().into_response_for(state, &uri));
                }
                Err(ZstdDecodeError::Overloaded) => {
                    return Err(PublicApiError::overloaded().into_response_for(state, &uri));
                }
                Err(ZstdDecodeError::Invalid) => {
                    return Err(PublicApiError::unreadable_body().into_response_for(state, &uri));
                }
                Err(ZstdDecodeError::AllocationFailed | ZstdDecodeError::TaskFailed) => {
                    return Err(PublicApiError::internal().into_response_for(state, &uri));
                }
            },
            ContentEncoding::Identity => bytes,
            ContentEncoding::Invalid => unreachable!("content encoding was validated"),
        };
        Ok(Self { headers, body })
    }
}

fn body_collection_rejection(
    error: BodyCollectionError,
    state: &AppState,
    uri: &axum::http::Uri,
) -> Response {
    match error {
        BodyCollectionError::TooLarge => {
            PublicApiError::payload_too_large().into_response_for(state, uri)
        }
        BodyCollectionError::Unreadable => {
            PublicApiError::unreadable_body().into_response_for(state, uri)
        }
        BodyCollectionError::AllocationFailed => {
            PublicApiError::internal().into_response_for(state, uri)
        }
    }
}

fn operation_for_path(path: &str) -> Option<ProtocolOperation> {
    let path = path
        .trim_end_matches('/')
        .trim_start_matches('/')
        .strip_prefix("v1/")
        .unwrap_or_else(|| path.trim_end_matches('/').trim_start_matches('/'));
    match path {
        "responses" => Some(ProtocolOperation::Responses),
        "responses/compact" => Some(ProtocolOperation::ResponsesCompact),
        "chat/completions" => Some(ProtocolOperation::ChatCompletions),
        "images/generations" => Some(ProtocolOperation::ImagesGenerations),
        "images/edits" => Some(ProtocolOperation::ImagesEdits),
        "messages" => Some(ProtocolOperation::Messages),
        "messages/count_tokens" => Some(ProtocolOperation::MessagesCountTokens),
        _ => None,
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

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::{ContentEncoding, content_encoding, operation_for_path, path_supports_zstd};

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
        assert!(!path_supports_zstd("/v1/images/generations"));
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
    fn public_body_paths_map_to_their_exact_operations() {
        assert_eq!(
            operation_for_path("/v1/images/edits"),
            Some(any2api_domain::ProtocolOperation::ImagesEdits)
        );
        assert_eq!(
            operation_for_path("/images/edits"),
            Some(any2api_domain::ProtocolOperation::ImagesEdits)
        );
        assert_eq!(
            operation_for_path("/v1/messages/count_tokens"),
            Some(any2api_domain::ProtocolOperation::MessagesCountTokens)
        );
        assert_eq!(operation_for_path("/v1/images/unknown"), None);
    }
}
