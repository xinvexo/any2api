use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::Response,
    routing::post,
};

use crate::{
    admin::{error::AdminApiError, no_store},
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
    payload: Result<Json<OAuthStartRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let request = parse_json(payload)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .start(request.provider())
        .await
        .map_err(error::map)?;
    Ok(no_store::json(OAuthStartResponse::from(result)))
}

async fn exchange(
    State(state): State<AppState>,
    payload: Result<Json<OAuthExchangeRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (session_id, callback_url) = parse_json(payload)?.into_parts();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .exchange(&session_id, &callback_url)
        .await
        .map_err(error::map)?;
    Ok(no_store::json(OAuthExchangeResponse::from(result)))
}

async fn poll_device(
    State(state): State<AppState>,
    payload: Result<Json<OAuthDevicePollRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let session_id = parse_json(payload)?.into_session_id();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service.poll_device(&session_id).await.map_err(error::map)?;
    Ok(no_store::json(OAuthDevicePollResponse::from(result)))
}

fn parse_json<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AdminApiError> {
    payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}
