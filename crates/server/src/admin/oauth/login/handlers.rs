use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use crate::{
    admin::{error::AdminApiError, request_json::AdminJson},
    state::AppState,
};

use super::{
    dto::{
        OAuthDevicePollRequest, OAuthDevicePollResponse, OAuthExchangeRequest,
        OAuthExchangeResponse, OAuthStartRequest, OAuthStartResponse,
    },
    error,
};

pub(in crate::admin::oauth) fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/start", post(start))
        .route("/oauth/exchange", post(exchange))
        .route("/oauth/device/poll", post(poll_device))
}

async fn start(
    State(state): State<AppState>,
    AdminJson(request): AdminJson<OAuthStartRequest>,
) -> Result<Json<OAuthStartResponse>, AdminApiError> {
    let (provider, proxy_selection) = request.into_parts();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .start(provider, proxy_selection)
        .await
        .map_err(error::map)?;
    Ok(Json(OAuthStartResponse::from(result)))
}

async fn exchange(
    State(state): State<AppState>,
    AdminJson(payload): AdminJson<OAuthExchangeRequest>,
) -> Result<Json<OAuthExchangeResponse>, AdminApiError> {
    let (session_id, callback_url) = payload.into_parts();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .exchange(&session_id, &callback_url)
        .await
        .map_err(error::map)?;
    Ok(Json(OAuthExchangeResponse::from(result)))
}

async fn poll_device(
    State(state): State<AppState>,
    AdminJson(payload): AdminJson<OAuthDevicePollRequest>,
) -> Result<Json<OAuthDevicePollResponse>, AdminApiError> {
    let session_id = payload.into_session_id();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.poll_device(&session_id).await.map_err(error::map)?;
    Ok(Json(OAuthDevicePollResponse::from(result)))
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}
