use std::sync::Arc;

use any2api_runtime::api::PublishedSnapshot;
use axum::{
    Json,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode, header::SET_COOKIE},
    response::{IntoResponse, Response},
};

use crate::{
    admin::request_json::AdminJson,
    admin_auth::{AdminAuthError, AuthenticatedAdminSession},
    client_address::{ClientAddressContext, ClientConnection},
    state::AppState,
};

use super::{
    access, cookie as auth_cookie,
    dto::{AdminSessionResponse, PasswordRequest, PasswordRotationRequest, SetupRequest},
    error::AdminApiError,
};

pub(super) async fn session(
    State(state): State<AppState>,
    Extension(context): Extension<ClientAddressContext>,
    headers: HeaderMap,
) -> Result<Json<AdminSessionResponse>, AdminApiError> {
    let (connection, snapshot) = access::resolve(context)?;
    let auth = state.admin_auth();
    let initialized = auth.is_initialized().await;
    let authenticated = if initialized {
        match auth_cookie::read(&headers)? {
            Some(token) => auth.authenticate(token, snapshot.settings().admin()).await,
            None => None,
        }
    } else {
        None
    };
    Ok(Json(AdminSessionResponse::new(
        initialized,
        authenticated.map(AuthenticatedAdminSession::csrf_token),
        snapshot.settings().admin().remote_enabled(),
        connection,
    )))
}

pub(super) async fn setup(
    State(state): State<AppState>,
    Extension(context): Extension<ClientAddressContext>,
    AdminJson(request): AdminJson<SetupRequest>,
) -> Result<Response, AdminApiError> {
    let (connection, snapshot) = access::resolve(context)?;
    if !connection.is_direct_loopback() {
        return Err(AdminApiError::setup_loopback_only());
    }
    let auth = state.admin_auth();
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
    Extension(context): Extension<ClientAddressContext>,
    AdminJson(request): AdminJson<PasswordRequest>,
) -> Result<Response, AdminApiError> {
    let (connection, snapshot) = access::resolve(context)?;
    let auth = state.admin_auth();
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
    Extension(connection): Extension<ClientConnection>,
    Extension(snapshot): Extension<Arc<PublishedSnapshot>>,
    AdminJson(request): AdminJson<PasswordRotationRequest>,
) -> Result<Response, AdminApiError> {
    let auth = state.admin_auth_handle();
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
    Extension(connection): Extension<ClientConnection>,
) -> Result<Response, AdminApiError> {
    state.admin_auth().logout(session).await;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(SET_COOKIE, auth_cookie::clear(connection.is_secure()));
    Ok(response)
}

fn session_response(
    issue: &crate::admin_auth::AdminSessionIssue,
    connection: ClientConnection,
    snapshot: &any2api_runtime::api::PublishedSnapshot,
) -> Result<Response, AdminApiError> {
    let settings = snapshot.settings().admin();
    let mut response = Json(AdminSessionResponse::new(
        true,
        Some(issue.csrf_token().to_owned()),
        settings.remote_enabled(),
        connection,
    ))
    .into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        auth_cookie::issue(
            issue.token(),
            connection.is_secure(),
            settings.session_absolute_timeout_secs(),
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
