use any2api_domain::{ProtocolDialect, PublicError, PublicErrorCode};
use axum::{
    extract::{Request, State},
    http::{HeaderValue, Uri, header::CACHE_CONTROL},
    response::Response,
};

use crate::state::AppState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicErrorKind {
    InvalidForwardedHeaders,
    Unauthorized,
    ConflictingCredentials,
    PayloadTooLarge,
    UnreadableBody,
    UnsupportedContentEncoding,
    Overloaded,
    Internal,
    NotFound,
    MethodNotAllowed,
}

#[derive(Debug)]
pub(crate) struct PublicApiError {
    kind: PublicErrorKind,
}

impl PublicApiError {
    pub(crate) const fn invalid_forwarded_headers() -> Self {
        Self {
            kind: PublicErrorKind::InvalidForwardedHeaders,
        }
    }

    pub(crate) const fn unauthorized() -> Self {
        Self {
            kind: PublicErrorKind::Unauthorized,
        }
    }

    pub(crate) const fn conflicting_credentials() -> Self {
        Self {
            kind: PublicErrorKind::ConflictingCredentials,
        }
    }

    pub(crate) const fn payload_too_large() -> Self {
        Self {
            kind: PublicErrorKind::PayloadTooLarge,
        }
    }

    pub(crate) const fn unreadable_body() -> Self {
        Self {
            kind: PublicErrorKind::UnreadableBody,
        }
    }

    pub(crate) const fn unsupported_content_encoding() -> Self {
        Self {
            kind: PublicErrorKind::UnsupportedContentEncoding,
        }
    }

    pub(crate) const fn overloaded() -> Self {
        Self {
            kind: PublicErrorKind::Overloaded,
        }
    }

    pub(crate) const fn internal() -> Self {
        Self {
            kind: PublicErrorKind::Internal,
        }
    }

    const fn not_found() -> Self {
        Self {
            kind: PublicErrorKind::NotFound,
        }
    }

    const fn method_not_allowed() -> Self {
        Self {
            kind: PublicErrorKind::MethodNotAllowed,
        }
    }

    pub(crate) fn into_response_for(self, state: &AppState, uri: &Uri) -> Response {
        self.into_response_for_dialect(state, dialect_for_uri(uri))
    }

    fn into_response_for_dialect(self, state: &AppState, dialect: ProtocolDialect) -> Response {
        let (code, message) = match self.kind {
            PublicErrorKind::InvalidForwardedHeaders => (
                PublicErrorCode::InvalidRequest,
                "trusted proxy headers are invalid",
            ),
            PublicErrorKind::Unauthorized => (
                PublicErrorCode::Unauthorized,
                "a valid Gateway API Key is required",
            ),
            PublicErrorKind::ConflictingCredentials => (
                PublicErrorCode::InvalidRequest,
                "authentication headers must contain the same Gateway API Key",
            ),
            PublicErrorKind::PayloadTooLarge => (
                PublicErrorCode::PayloadTooLarge,
                "request body exceeds the public API request size limit",
            ),
            PublicErrorKind::UnreadableBody => (
                PublicErrorCode::InvalidRequest,
                "request body could not be read",
            ),
            PublicErrorKind::UnsupportedContentEncoding => (
                PublicErrorCode::InvalidRequest,
                "content-encoding is not supported for this public API route",
            ),
            PublicErrorKind::Overloaded => (
                PublicErrorCode::LocalRateLimit,
                "the server is too busy to decompress this request; retry shortly",
            ),
            PublicErrorKind::Internal => (
                PublicErrorCode::InternalError,
                "internal request body processing failed",
            ),
            PublicErrorKind::NotFound => (
                PublicErrorCode::PublicApiNotFound,
                "public API route was not found",
            ),
            PublicErrorKind::MethodNotAllowed => (
                PublicErrorCode::MethodNotAllowed,
                "request method is not allowed for this public API route",
            ),
        };
        let public_error = PublicError::new(code, message);
        let mut response = super::response::from_runtime(
            state
                .public_requests()
                .error_response(dialect, &public_error),
        );
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

pub(crate) async fn not_found(State(state): State<AppState>, request: Request) -> Response {
    PublicApiError::not_found().into_response_for(&state, request.uri())
}

pub(crate) async fn method_not_allowed(
    State(state): State<AppState>,
    request: Request,
) -> Response {
    PublicApiError::method_not_allowed().into_response_for(&state, request.uri())
}

fn dialect_for_uri(uri: &Uri) -> ProtocolDialect {
    let path = uri
        .path()
        .trim_start_matches('/')
        .strip_prefix("v1/")
        .unwrap_or_else(|| uri.path().trim_start_matches('/'));
    if path == "messages" || path.starts_with("messages/") {
        ProtocolDialect::AnthropicMessages
    } else if path == "chat" || path.starts_with("chat/") {
        ProtocolDialect::OpenAiChatCompletions
    } else if path == "images" || path.starts_with("images/") {
        ProtocolDialect::OpenAiImages
    } else {
        ProtocolDialect::OpenAiResponses
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolDialect;
    use axum::http::Uri;

    use super::dialect_for_uri;

    #[test]
    fn messages_paths_use_anthropic_and_other_paths_use_openai() {
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/messages")),
            ProtocolDialect::AnthropicMessages
        );
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/messages/count_tokens")),
            ProtocolDialect::AnthropicMessages
        );
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/v1/messages/count_tokens")),
            ProtocolDialect::AnthropicMessages
        );
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/v1/chat/completions")),
            ProtocolDialect::OpenAiChatCompletions
        );
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/v1/images/generations")),
            ProtocolDialect::OpenAiImages
        );
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/responses")),
            ProtocolDialect::OpenAiResponses
        );
        assert_eq!(
            dialect_for_uri(&Uri::from_static("/not-a-route")),
            ProtocolDialect::OpenAiResponses
        );
    }
}
