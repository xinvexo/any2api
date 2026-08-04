use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    admin::{error::AdminApiError, request_json::AdminJson},
    state::AppState,
};

use super::{
    super::account,
    dto::{OAuthQuotaResetRequest, OAuthQuotaResetResponse, OAuthQuotaResponse},
    error,
};

pub(in crate::admin::oauth) fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/accounts/{id}/quota", get(cached_quota))
        .route("/oauth/accounts/{id}/quota/refresh", post(refresh_quota))
        .route("/oauth/accounts/{id}/quota/reset", post(reset_quota))
}

async fn cached_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<OAuthQuotaResponse>>, AdminApiError> {
    let id = account::parse_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.cached_quota(id).await.map_err(error::map)?;
    Ok(Json(result.map(OAuthQuotaResponse::from)))
}

async fn refresh_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<OAuthQuotaResponse>, AdminApiError> {
    let id = account::parse_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.refresh_quota(id).await.map_err(error::map)?;
    Ok(Json(OAuthQuotaResponse::from(result)))
}

async fn reset_quota(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(request): AdminJson<OAuthQuotaResetRequest>,
) -> Result<Json<OAuthQuotaResetResponse>, AdminApiError> {
    let id = account::parse_id(&id)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .reset_quota(id, request.into_redeem_request_id())
        .await
        .map_err(error::map)?;
    Ok(Json(OAuthQuotaResetResponse::from(result)))
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}
