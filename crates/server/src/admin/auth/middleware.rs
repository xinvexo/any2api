use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{client_address::ClientAddressContext, state::AppState};

use super::{access, cookie as auth_cookie, error::AdminApiError};

pub(in crate::admin) async fn require_admin_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let auth = state.admin_auth();
    let Some(context) = request.extensions().get::<ClientAddressContext>().cloned() else {
        tracing::error!("admin request client-address context is missing");
        return AdminApiError::internal().into_response();
    };
    let (connection, snapshot) = match access::resolve(context) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let token = match auth_cookie::read(request.headers()) {
        Ok(Some(token)) => token,
        Ok(None) | Err(_) => return AdminApiError::session_required().into_response(),
    };
    let Some(session) = auth.authenticate(token, snapshot.settings().admin()).await else {
        return AdminApiError::session_required().into_response();
    };
    if requires_csrf(request.method()) {
        let csrf = request
            .headers()
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok());
        if !csrf.is_some_and(|csrf| session.csrf_matches(csrf)) {
            return AdminApiError::csrf_invalid().into_response();
        }
    }
    request.extensions_mut().insert(connection);
    request.extensions_mut().insert(snapshot);
    request.extensions_mut().insert(session);
    next.run(request).await
}

fn requires_csrf(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}
