use axum::{
    Json,
    body::Body,
    extract::{State, rejection::JsonRejection},
    http::{
        HeaderValue, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS},
    },
    response::{IntoResponse, Response},
};

use crate::state::AppState;

use super::{
    error::AdminApiError,
    no_store,
    oauth_dto::{OAuthExchangeRequest, OAuthStartRequest, OAuthStartResponse},
    oauth_error,
};

pub(super) async fn start(
    State(state): State<AppState>,
    payload: Result<Json<OAuthStartRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let request = parse_json(payload)?;
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let result = service
        .start(request.provider())
        .await
        .map_err(oauth_error::map)?;
    Ok(no_store::json(OAuthStartResponse::from(result)))
}

pub(super) async fn exchange(
    State(state): State<AppState>,
    payload: Result<Json<OAuthExchangeRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (session_id, callback_url) = parse_json(payload)?.into_parts();
    let service = state.oauth().ok_or_else(oauth_unavailable)?;
    let download = service
        .exchange(&session_id, &callback_url)
        .await
        .map_err(oauth_error::map)?;
    let disposition =
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", download.filename()))
            .map_err(|_| oauth_error::map(any2api_runtime::api::OAuthError::FileSerialization))?;
    let mut response = (StatusCode::OK, Body::from(download.into_bytes())).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    response
        .headers_mut()
        .insert(CONTENT_DISPOSITION, disposition);
    response
        .headers_mut()
        .insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    Ok(response)
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
