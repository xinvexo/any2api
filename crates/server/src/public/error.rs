use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::CACHE_CONTROL},
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct PublicApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

impl PublicApiError {
    pub(crate) const fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "a valid Gateway API Key is required",
        }
    }

    pub(crate) const fn conflicting_credentials() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "invalid_request",
            message: "authentication headers must contain the same Gateway API Key",
        }
    }

    pub(crate) const fn not_implemented() -> Self {
        Self {
            status: StatusCode::NOT_IMPLEMENTED,
            code: "public_api_not_implemented",
            message: "this public protocol endpoint is not implemented yet",
        }
    }

    pub(crate) const fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "public_api_not_found",
            message: "public API route was not found",
        }
    }
}

impl IntoResponse for PublicApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

pub(crate) async fn not_found() -> PublicApiError {
    PublicApiError::not_found()
}
