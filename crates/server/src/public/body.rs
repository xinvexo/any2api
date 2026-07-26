use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

use super::error::PublicApiError;

/// Buffered public request body whose rejection uses the dialect-aware
/// protocol error envelope instead of axum's plain-text response.
pub(super) struct PublicBody(pub(super) Bytes);

impl FromRequest<AppState> for PublicBody {
    type Rejection = Response;

    async fn from_request(request: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        let uri = request.uri().clone();
        match Bytes::from_request(request, state).await {
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
