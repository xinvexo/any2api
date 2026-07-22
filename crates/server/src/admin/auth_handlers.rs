use std::{net::SocketAddr, sync::Arc};

use any2api_runtime::api::PublishedSnapshot;
use axum::{
    Json,
    extract::{ConnectInfo, Extension, State, rejection::JsonRejection},
    http::{
        HeaderMap, StatusCode,
        header::{CACHE_CONTROL, SET_COOKIE},
    },
    response::{IntoResponse, Response},
};

use crate::{
    admin_auth::{AdminAuthError, AdminConnection, AuthenticatedAdminSession},
    state::AppState,
};

use super::{
    access, auth_cookie,
    auth_dto::{AdminSessionResponse, PasswordRequest, PasswordRotationRequest, SetupRequest},
    error::AdminApiError,
    no_store,
};

pub(super) async fn session(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Response, AdminApiError> {
    let (connection, snapshot) = access::resolve(&state, Some(peer), &headers)?;
    let auth = state
        .admin_auth()
        .ok_or_else(AdminApiError::auth_unavailable)?;
    let initialized = auth.is_initialized().await;
    let authenticated = if initialized {
        match auth_cookie::read(&headers)? {
            Some(token) => auth.authenticate(token, snapshot.settings().admin()).await,
            None => None,
        }
    } else {
        None
    };
    Ok(no_store::json(AdminSessionResponse::new(
        initialized,
        authenticated.map(AuthenticatedAdminSession::csrf_token),
        snapshot.settings().admin().remote_enabled(),
        connection,
    )))
}

pub(super) async fn setup(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    payload: Result<Json<SetupRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (connection, snapshot) = access::resolve(&state, Some(peer), &headers)?;
    if !connection.is_loopback() {
        return Err(AdminApiError::setup_loopback_only());
    }
    let request = payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))?;
    let auth = state
        .admin_auth()
        .ok_or_else(AdminApiError::auth_unavailable)?;
    if !auth
        .initialize_with_setup_token(request.password.clone(), &request.setup_token)
        .await
        .map_err(map_auth_error)?
    {
        return Err(AdminApiError::already_initialized());
    }
    let issue = auth
        .login(
            request.password,
            connection.client_ip(),
            snapshot.settings().admin(),
        )
        .await
        .map_err(map_auth_error)?;
    session_response(&issue, connection, &snapshot)
}

pub(super) async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    payload: Result<Json<PasswordRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let (connection, snapshot) = access::resolve(&state, Some(peer), &headers)?;
    let request = payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))?;
    let auth = state
        .admin_auth()
        .ok_or_else(AdminApiError::auth_unavailable)?;
    let issue = auth
        .login(
            request.password,
            connection.client_ip(),
            snapshot.settings().admin(),
        )
        .await
        .map_err(map_auth_error)?;
    session_response(&issue, connection, &snapshot)
}

pub(super) async fn rotate_password(
    State(state): State<AppState>,
    Extension(connection): Extension<AdminConnection>,
    Extension(snapshot): Extension<Arc<PublishedSnapshot>>,
    payload: Result<Json<PasswordRotationRequest>, JsonRejection>,
) -> Result<Response, AdminApiError> {
    let request = payload
        .map(|Json(value)| value)
        .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))?;
    let auth = state
        .admin_auth_handle()
        .ok_or_else(AdminApiError::auth_unavailable)?;
    let issue = state
        .runtime()
        .lifecycle()
        .spawn_critical(async move {
            auth.rotate_password(request.current_password, request.new_password)
                .await
        })
        .await
        .map_err(|error| {
            tracing::error!(error = ?error, "administrator password rotation task failed");
            AdminApiError::internal()
        })?
        .ok_or_else(AdminApiError::shutting_down)?
        .map_err(map_auth_error)?;
    session_response(&issue, connection, &snapshot)
}

pub(super) async fn logout(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedAdminSession>,
    Extension(connection): Extension<AdminConnection>,
) -> Result<Response, AdminApiError> {
    state
        .admin_auth()
        .ok_or_else(AdminApiError::auth_unavailable)?
        .logout(session)
        .await;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, auth_cookie::clear(connection.is_secure()));
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

fn session_response(
    issue: &crate::admin_auth::AdminSessionIssue,
    connection: AdminConnection,
    snapshot: &any2api_runtime::api::PublishedSnapshot,
) -> Result<Response, AdminApiError> {
    let settings = snapshot.settings().admin();
    let mut response = no_store::json(AdminSessionResponse::new(
        true,
        Some(issue.csrf_token().to_owned()),
        settings.remote_enabled(),
        connection,
    ));
    response.headers_mut().insert(
        SET_COOKIE,
        auth_cookie::issue(
            issue.token(),
            connection.is_secure(),
            settings.session_absolute_timeout_ms(),
        )?,
    );
    Ok(response)
}

fn map_auth_error(error: AdminAuthError) -> AdminApiError {
    match error {
        AdminAuthError::InvalidPassword => AdminApiError::invalid_admin_password(),
        AdminAuthError::InvalidSetupToken => AdminApiError::invalid_setup_token(),
        AdminAuthError::NotInitialized => AdminApiError::setup_required(),
        AdminAuthError::InvalidCredentials => AdminApiError::invalid_credentials(),
        AdminAuthError::CurrentPasswordInvalid => AdminApiError::current_password_invalid(),
        AdminAuthError::CredentialChanged => AdminApiError::password_changed(),
        AdminAuthError::RateLimited { retry_after } => {
            AdminApiError::login_rate_limited(retry_after)
        }
        internal => {
            tracing::error!(error = ?internal, "administrator authentication failed");
            AdminApiError::internal()
        }
    }
}
