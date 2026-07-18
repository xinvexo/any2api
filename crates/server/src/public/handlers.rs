use axum::{extract::Extension, response::Response};

use super::{auth::AuthenticatedGatewayApiKey, error::PublicApiError};

pub(crate) async fn not_implemented(
    Extension(authenticated): Extension<AuthenticatedGatewayApiKey>,
) -> Result<Response, PublicApiError> {
    let _ = (authenticated.id(), authenticated.snapshot().revision());
    Err(PublicApiError::not_implemented())
}
