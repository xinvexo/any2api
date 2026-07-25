use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Response,
    routing::{get, post},
};

use crate::{
    admin::{error::AdminApiError, no_store},
    state::AppState,
};

use super::{
    super::account,
    dto::{OAuthQuotaResetResponse, OAuthQuotaResponse},
    error,
};

pub(in crate::admin::oauth) fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/accounts/{id}/quota", get(quota))
        .route("/oauth/accounts/{id}/quota/reset", post(reset_quota))
}

async fn quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let id = account::parse_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.query_quota(id).await.map_err(error::map)?;
    Ok(no_store::json(OAuthQuotaResponse::from(result)))
}

async fn reset_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let id = account::parse_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.reset_quota(id).await.map_err(error::map)?;
    Ok(no_store::json(OAuthQuotaResetResponse::from(result)))
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}
